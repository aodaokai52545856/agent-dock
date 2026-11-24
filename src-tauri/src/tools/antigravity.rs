use super::{matches_cwd, truncate_title, RenameKind, SessionRow, ToolId};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
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
    let mut pending: HashSet<String> = fs::read_dir(root.join("conversations"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(".db")?;
            canonical_id(id).then(|| id.to_owned())
        })
        .collect();
    let database = read_summaries(&root.join("conversation_summaries.db"), &cache, cwd);
    let database_available = database.is_ok();
    let (mut rows, indexed) = database.unwrap_or_default();
    pending.retain(|id| !indexed.contains(id));
    for (id, entry) in cache.conversations {
        // With a healthy index, recover only conversations that still exist on disk.
        if database_available && !pending.contains(&id) {
            continue;
        }
        if hidden_entry(&entry) {
            pending.remove(&id);
            continue;
        }
        let Ok(summary) = CachedSummary::deserialize(&entry["summary"]) else {
            continue;
        };
        if summary.app_data_dir != CLI_APP_DATA {
            pending.remove(&id);
            continue;
        }
        let uris = summary.workspace_uris.unwrap_or_default();
        if !uris.is_empty() {
            pending.remove(&id);
            if let Some(row) = session_row(&id, &summary.title, &summary.updated_at, &uris, cwd) {
                rows.push(row);
            }
        }
    }
    if !pending.is_empty() {
        recover_input_history(&root.join("history.jsonl"), &pending, cwd, &mut rows);
    }
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
) -> rusqlite::Result<(Vec<SessionRow>, HashSet<String>)> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_millis(150))?;
    let mut stmt = conn.prepare(
        "SELECT conversation_id, COALESCE(title, ''), COALESCE(last_modified_time, ''),
                COALESCE(workspace_uris, ''), app_data_dir = ?1 AND COALESCE(parent_conversation_id, '') = ''
         FROM conversation_summaries",
    )?;
    let mut results = stmt.query([CLI_APP_DATA])?;
    let mut rows = Vec::new();
    let mut indexed = HashSet::new();
    while let Some(result) = results.next()? {
        let (Ok(id), Ok(title), Ok(updated_at), Ok(uris)) = (
            result.get::<_, String>(0),
            result.get::<_, String>(1),
            result.get::<_, String>(2),
            result.get::<_, String>(3),
        ) else {
            continue;
        };
        if !result.get::<_, bool>(4).unwrap_or(false)
            || cache.conversations.get(&id).is_some_and(hidden_entry)
        {
            indexed.insert(id);
            continue;
        }
        let Ok(uris) = serde_json::from_str::<Vec<String>>(&uris) else {
            continue;
        };
        if uris.is_empty() {
            continue;
        }
        if let Some(row) = session_row(&id, &title, &updated_at, &uris, cwd) {
            rows.push(row);
        }
        indexed.insert(id);
    }
    Ok((rows, indexed))
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

fn canonical_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|uuid| uuid.hyphenated().to_string() == id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    conversation_id: String,
    workspace: String,
    timestamp: i64,
}

struct InputHistory {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
    entries: HashMap<String, HistoryEntry>,
}

// Read agy's input index only for unindexed conversation files. Cache the tiny
// per-ID metadata across project refreshes; never open the conversation bodies.
fn recover_input_history(
    path: &Path,
    pending: &HashSet<String>,
    cwd: &str,
    rows: &mut Vec<SessionRow>,
) {
    static LAST: Mutex<Option<InputHistory>> = Mutex::new(None);
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    let Ok(metadata) = file.metadata() else {
        return;
    };
    let Ok(modified) = metadata.modified() else {
        return;
    };
    let mut cache = LAST.lock().unwrap_or_else(|err| err.into_inner());
    if !cache
        .as_ref()
        .is_some_and(|h| h.path == path && h.length == metadata.len() && h.modified == modified)
    {
        let mut entries = HashMap::new();
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Err(_) => return,
                Ok(_) => {}
            }
            if let Ok(entry) = serde_json::from_slice::<HistoryEntry>(&line) {
                entries.insert(entry.conversation_id.clone(), entry);
            }
        }
        *cache = Some(InputHistory {
            path: path.to_owned(),
            length: metadata.len(),
            modified,
            entries,
        });
    }
    if let Some(history) = cache.as_ref() {
        for id in pending {
            if let Some(entry) = history
                .entries
                .get(id)
                .filter(|entry| matches_cwd(&entry.workspace, cwd))
            {
                rows.push(make_row(id, "", entry.timestamp, cwd));
            }
        }
    }
}

fn session_row(
    id: &str,
    title: &str,
    updated_at: &str,
    uris: &[String],
    cwd: &str,
) -> Option<SessionRow> {
    if !canonical_id(id) || !uris.iter().any(|uri| belongs_to_workspace(uri, cwd)) {
        return None;
    }
    let updated_at = chrono::DateTime::parse_from_rfc3339(updated_at)
        .or_else(|_| chrono::DateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M:%S%.f%:z"))
        .map(|time| time.timestamp_millis())
        .unwrap_or_default();
    Some(make_row(id, title, updated_at, cwd))
}

fn make_row(id: &str, title: &str, updated_at: i64, cwd: &str) -> SessionRow {
    let title = if title.trim().is_empty() {
        format!("Antigravity · {}", &id[..8])
    } else {
        truncate_title(title, 80)
    };
    SessionRow {
        profile_id: None,
        tool_id: ToolId::Antigravity,
        id: id.to_owned(),
        title,
        cwd: cwd.to_owned(),
        updated_at,
        rename_kind: RenameKind::Overlay,
        account: None,
    }
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
        let conversations = dir.path().join("conversations");
        fs::create_dir(&conversations).unwrap();
        // A child also has its own conversation file and can have stale cache metadata.
        fs::write(conversations.join(format!("{OLDER}.db")), []).unwrap();
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
        fs::remove_file(conversations.join(format!("{OLDER}.db"))).unwrap();
        assert!(list_sessions_in(dir.path(), WORKSPACE).is_empty());
    }

    #[test]
    fn recovers_closed_sessions_by_exact_id_from_cache_and_input_index() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let conversations = root.join("conversations");
        fs::create_dir(&conversations).unwrap();
        for id in [RECENT, OLDER, FOREIGN] {
            fs::write(conversations.join(format!("{id}.db")), []).unwrap();
        }
        let conn = Connection::open(root.join("conversation_summaries.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversation_summaries (conversation_id, title, last_modified_time, workspace_uris, app_data_dir, parent_conversation_id);",
        ).unwrap();
        let cache = root.join("cache");
        fs::create_dir(&cache).unwrap();
        let gone = "dddddddd-1234-1234-1234-123456789abc";
        let summary = json!({"summary": {
            "Title": "Cached title", "WorkspaceURIs": [WORKSPACE_URI], "AppDataDir": CLI_APP_DATA
        }});
        fs::write(
            cache.join("conversation_metadata.json"),
            json!({"conversations": {
                (RECENT): summary, (gone): summary,
            }})
            .to_string(),
        )
        .unwrap();
        let history_path = root.join("history.jsonl");
        let input = |id, cwd, at| {
            json!({"conversationId": id, "workspace": cwd, "timestamp": at}).to_string()
        };
        let history = [
            input(OLDER, WORKSPACE, 100),
            "incomplete or malformed record".into(),
            input(gone, WORKSPACE, 200),
            input(FOREIGN, "/different/project", 300),
            input(OLDER, WORKSPACE, 400),
        ]
        .join("\n");
        fs::write(&history_path, history).unwrap();
        let rows = list_sessions_in(root, WORKSPACE);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            [OLDER, RECENT]
        );
        assert_eq!(rows[0].updated_at, 400);
        assert_eq!(rows[1].title, "Cached title");

        fs::write(&history_path, input(FOREIGN, WORKSPACE, 500)).unwrap();
        let rows = list_sessions_in(root, WORKSPACE);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            [FOREIGN, RECENT]
        );
    }
}
