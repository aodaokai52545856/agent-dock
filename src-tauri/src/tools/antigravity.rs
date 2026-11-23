use super::{matches_cwd, truncate_title, RenameKind, SessionRow, ToolId};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tauri::Url;
use uuid::Uuid;

const CLI_APP_DATA: &str = "antigravity-cli";

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    let Some(home) = dirs::home_dir() else {
        return Ok(Vec::new());
    };
    Ok(list_sessions_in(
        &home.join(".gemini").join(CLI_APP_DATA),
        cwd,
    ))
}

fn list_sessions_in(root: &Path, cwd: &str) -> Vec<SessionRow> {
    let cache = read_cache(&root.join("cache").join("conversation_metadata.json"));
    let mut rows = read_summaries(&root.join("conversation_summaries.db"), &cache, cwd)
        .unwrap_or_else(|_| cached_sessions(cache, cwd));
    rows.sort_unstable_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows
}

fn read_summaries(
    path: &Path,
    cache: &MetadataCache,
    cwd: &str,
) -> rusqlite::Result<Vec<SessionRow>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_millis(150))?;
    let mut stmt = conn.prepare(
        "SELECT conversation_id, COALESCE(title, ''), COALESCE(last_modified_time, ''), workspace_uris
         FROM conversation_summaries
         WHERE app_data_dir = ?1 AND COALESCE(parent_conversation_id, '') = ''",
    )?;
    let mut results = stmt.query([CLI_APP_DATA])?;
    let mut rows = Vec::new();
    while let Some(result) = results.next()? {
        let (Ok(id), Ok(title), Ok(updated_at), Ok(uris)) = (
            result.get::<_, String>(0),
            result.get::<_, String>(1),
            result.get::<_, String>(2),
            result.get::<_, String>(3),
        ) else {
            continue;
        };
        if cache.conversations.get(&id).is_some_and(hidden_entry) {
            continue;
        }
        let Ok(uris) = serde_json::from_str::<Vec<String>>(&uris) else {
            continue;
        };
        if let Some(row) = session_row(&id, &title, &updated_at, &uris, cwd) {
            rows.push(row);
        }
    }
    Ok(rows)
}

#[derive(Default, Deserialize)]
struct MetadataCache {
    conversations: HashMap<String, Value>,
}

fn hidden_entry(entry: &Value) -> bool {
    entry["is_internal"] == true
        || entry["summary"]["Internal"] == true
        || entry["summary"]["Cloud"] == true
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CachedSummary {
    #[serde(default)]
    title: String,
    #[serde(default)]
    updated_at: String,
    #[serde(rename = "WorkspaceURIs")]
    workspace_uris: Option<Vec<String>>,
    app_data_dir: String,
}

fn read_cache(path: &Path) -> MetadataCache {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

fn cached_sessions(cache: MetadataCache, cwd: &str) -> Vec<SessionRow> {
    cache
        .conversations
        .into_iter()
        .filter_map(|(id, entry)| {
            if hidden_entry(&entry) {
                return None;
            }
            let summary = CachedSummary::deserialize(entry.get("summary")?).ok()?;
            if summary.app_data_dir != CLI_APP_DATA {
                return None;
            }
            session_row(
                &id,
                &summary.title,
                &summary.updated_at,
                summary.workspace_uris.as_deref()?,
                cwd,
            )
        })
        .collect()
}

fn session_row(
    id: &str,
    title: &str,
    updated_at: &str,
    uris: &[String],
    cwd: &str,
) -> Option<SessionRow> {
    let uuid = Uuid::parse_str(id).ok()?;
    if uuid.hyphenated().to_string() != id || !uris.iter().any(|uri| belongs_to_workspace(uri, cwd))
    {
        return None;
    }
    let updated_at = chrono::DateTime::parse_from_rfc3339(updated_at)
        .or_else(|_| chrono::DateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M:%S%.f%:z"))
        .map(|time| time.timestamp_millis())
        .unwrap_or_default();
    let title = if title.trim().is_empty() {
        format!("Antigravity · {}", &id[..8])
    } else {
        truncate_title(title, 80)
    };
    Some(SessionRow {
        profile_id: None,
        tool_id: ToolId::Antigravity,
        id: id.to_owned(),
        title,
        cwd: cwd.to_owned(),
        updated_at,
        rename_kind: RenameKind::Overlay,
        account: None,
    })
}

fn belongs_to_workspace(uri: &str, cwd: &str) -> bool {
    Url::parse(uri)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .and_then(|path| path.into_os_string().into_string().ok())
        .is_some_and(|path| matches_cwd(&path, cwd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    const RECENT: &str = "12345678-1234-1234-1234-123456789abc";
    const OLDER: &str = "00000000-1234-1234-1234-123456789abc";
    const FOREIGN: &str = "55555555-1234-1234-1234-123456789abc";

    #[cfg(windows)]
    const WORKSPACE_URI: &str = "file:///D:/Projects/Dock%20Dev";
    #[cfg(windows)]
    const WORKSPACE: &str = r"d:\projects\dock dev\";
    #[cfg(not(windows))]
    const WORKSPACE_URI: &str = "file:///projects/dock%20dev";
    #[cfg(not(windows))]
    const WORKSPACE: &str = "/projects/dock dev";

    #[test]
    fn reads_workspace_history_with_cli_cache_when_database_is_missing() {
        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();
        let entry = |at: &str, uri: &str, source: &str, cloud| {
            json!({"summary": {
                "Title": "Local conversation", "UpdatedAt": at, "WorkspaceURIs": [uri],
                "AppDataDir": source, "Cloud": cloud
            }})
        };
        let recent = "2026-11-23T03:04:05Z";
        let older = "2026-11-20T03:04:05Z";
        let metadata = json!({"conversations": {
            (RECENT): entry(recent, WORKSPACE_URI, CLI_APP_DATA, false),
            (OLDER): entry(older, WORKSPACE_URI, CLI_APP_DATA, false),
            (FOREIGN): entry(recent, &format!("{WORKSPACE_URI}/another"), CLI_APP_DATA, false),
            "a2345678-1234-1234-1234-123456789abc": entry(recent, WORKSPACE_URI, "antigravity", false),
            "b2345678-1234-1234-1234-123456789abc": entry(recent, WORKSPACE_URI, CLI_APP_DATA, true),
            "c2345678-1234-1234-1234-123456789abc": {"is_internal": true, "summary": null},
            "invalid": entry(recent, WORKSPACE_URI, CLI_APP_DATA, false),
        }});
        fs::write(
            cache_dir.join("conversation_metadata.json"),
            metadata.to_string(),
        )
        .unwrap();

        let rows = list_sessions_in(dir.path(), WORKSPACE);
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            [RECENT, OLDER]
        );
        assert_eq!(rows[0].title, "Local conversation");
        assert_eq!(rows[0].rename_kind, RenameKind::Overlay);
        assert!(rows[0].updated_at > rows[1].updated_at);
    }

    #[test]
    fn prefers_read_only_database_and_excludes_nested_sessions() {
        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();
        let hidden_id = "aaaaaaaa-1234-1234-1234-123456789abc";
        let metadata = json!({"conversations": {
            (OLDER): {"summary": {
                "Title": "Stale local conversation",
                "UpdatedAt": "2026-11-20T03:04:05Z",
                "WorkspaceURIs": [WORKSPACE_URI],
                "AppDataDir": CLI_APP_DATA,
            }},
            (hidden_id): {"summary": {"Cloud": true}}
        }});
        fs::write(
            cache_dir.join("conversation_metadata.json"),
            metadata.to_string(),
        )
        .unwrap();
        let path = dir.path().join("conversation_summaries.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversation_summaries (
                conversation_id TEXT, title TEXT, last_modified_time DATETIME,
                workspace_uris TEXT, app_data_dir TEXT, parent_conversation_id TEXT
             );",
        )
        .unwrap();
        let uris = json!([WORKSPACE_URI]).to_string();
        for (id, parent, data_dir) in [
            (RECENT, "", CLI_APP_DATA),
            (OLDER, RECENT, CLI_APP_DATA),
            (FOREIGN, "", "antigravity"),
            (hidden_id, "", CLI_APP_DATA),
            ("invalid", "", CLI_APP_DATA),
        ] {
            conn.execute(
                "INSERT INTO conversation_summaries VALUES (?1, '', '2026-11-23 03:04:05.1234567+00:00', ?2, ?3, ?4)",
                rusqlite::params![id, &uris, data_dir, parent],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO conversation_summaries VALUES ('bad row', NULL, NULL, NULL, ?1, '')",
            [CLI_APP_DATA],
        )
        .unwrap();
        drop(conn);

        let rows = list_sessions_in(dir.path(), WORKSPACE);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, RECENT);
        assert_eq!(rows[0].title, "Antigravity · 12345678");
        assert_eq!(rows[0].updated_at, 1_795_403_045_123);

        let conn = Connection::open(path).unwrap();
        conn.execute("DELETE FROM conversation_summaries", [])
            .unwrap();
        drop(conn);
        assert!(list_sessions_in(dir.path(), WORKSPACE).is_empty());
    }
}
