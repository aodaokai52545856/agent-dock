use super::{
    RenameKind, SessionRow, ToolId, file_mtime_millis, first_string, matches_cwd, parse_time_value,
    truncate_title,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub fn kimi_home() -> PathBuf {
    std::env::var_os("KIMI_CODE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".kimi-code")
        })
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    let index = kimi_home().join("session_index.jsonl");
    if !index.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&index).map_err(|err| format!("无法读取 Kimi session 索引：{err}"))?;
    let mut rows = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|err| {
            format!("Kimi session_index.jsonl 第 {} 行不是 JSON：{err}", line_no + 1)
        })?;
        let work_dir = first_string(&value, &["workDir", "work_dir", "cwd"]).unwrap_or_default();
        if !work_dir.is_empty() && !matches_cwd(&work_dir, cwd) {
            continue;
        }
        let id = first_string(&value, &["sessionId", "session_id", "id"]).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let session_dir = first_string(&value, &["sessionDir", "session_dir"])
            .map(PathBuf::from)
            .unwrap_or_else(|| kimi_home().join("sessions").join(&id));
        if let Some(row) = read_state(&session_dir, &id, &work_dir, cwd) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(rows)
}

pub fn rename_session(session_id: &str, title: &str) -> Result<(), String> {
    let path = find_state(session_id).ok_or_else(|| {
        "没有找到这个 Kimi session 的 state.json，无法改名。请确认 ~/.kimi-code/sessions 还在。"
            .to_string()
    })?;
    let text = fs::read_to_string(&path).map_err(|err| format!("读取 state.json 失败：{err}"))?;
    let mut value: Value =
        serde_json::from_str(&text).map_err(|err| format!("state.json 不是合法 JSON：{err}"))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "state.json 根节点必须是对象".to_string())?;
    obj.insert("title".into(), json!(title));
    obj.insert("custom_title".into(), json!(title));
    obj.insert("isCustomTitle".into(), json!(true));
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&value).unwrap())
        .map_err(|err| format!("写入标题失败：{err}"))?;
    fs::rename(&tmp, &path).map_err(|err| format!("保存标题失败：{err}"))?;
    Ok(())
}

pub fn delete_session(session_id: &str) -> Result<(), String> {
    let index = kimi_home().join("session_index.jsonl");
    if !index.is_file() {
        return Err("没有找到 Kimi session 索引，无法删除。".into());
    }
    let text = fs::read_to_string(&index).map_err(|err| format!("读取 Kimi session 索引失败：{err}"))?;
    let mut kept = Vec::new();
    let mut session_dir = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("Kimi session_index.jsonl 不是 JSON：{err}"))?;
        let id = first_string(&value, &["sessionId", "session_id", "id"]).unwrap_or_default();
        if id == session_id {
            session_dir = first_string(&value, &["sessionDir", "session_dir"])
                .map(PathBuf::from)
                .or_else(|| Some(kimi_home().join("sessions").join(id)));
            continue;
        }
        kept.push(line.to_string());
    }
    if session_dir.is_none() {
        return Err("没有找到这个 Kimi session，无法删除。请确认 ~/.kimi-code 还在。".into());
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    let tmp = index.with_extension("jsonl.tmp");
    fs::write(&tmp, out).map_err(|err| format!("写入 Kimi session 索引失败：{err}"))?;
    fs::rename(&tmp, &index).map_err(|err| format!("保存 Kimi session 索引失败：{err}"))?;
    if let Some(dir) = session_dir {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|err| format!("删除 Kimi session 目录失败：{err}"))?;
        }
    }
    Ok(())
}

fn find_state(session_id: &str) -> Option<PathBuf> {
    let index = kimi_home().join("session_index.jsonl");
    let text = fs::read_to_string(index).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).ok()?;
        let id = first_string(&value, &["sessionId", "session_id", "id"])?;
        if id != session_id {
            continue;
        }
        if let Some(dir) = first_string(&value, &["sessionDir", "session_dir"]) {
            let path = PathBuf::from(dir).join("state.json");
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn read_state(session_dir: &Path, id: &str, work_dir: &str, project_cwd: &str) -> Option<SessionRow> {
    let path = session_dir.join("state.json");
    if !path.is_file() {
        return Some(SessionRow {
            tool_id: ToolId::Kimi,
            id: id.to_string(),
            title: id.to_string(),
            cwd: if work_dir.is_empty() {
                project_cwd.to_string()
            } else {
                work_dir.to_string()
            },
            updated_at: file_mtime_millis(session_dir),
            rename_kind: RenameKind::Native,
        });
    }
    let text = fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let custom = value.get("isCustomTitle").and_then(|v| v.as_bool()).unwrap_or(false);
    let title = if custom {
        first_string(&value, &["custom_title", "title"])
    } else {
        None
    }
    .or_else(|| first_string(&value, &["custom_title", "title"]))
    .or_else(|| first_string(&value, &["lastPrompt", "last_prompt"]).map(|s| truncate_title(&s, 36)))
    .unwrap_or_else(|| id.to_string());
    let updated = parse_time_value(value.get("updatedAt").unwrap_or(&Value::Null))
        .or_else(|| parse_time_value(value.get("updated_at").unwrap_or(&Value::Null)))
        .or_else(|| parse_time_value(value.get("createdAt").unwrap_or(&Value::Null)))
        .unwrap_or_else(|| file_mtime_millis(&path));
    let cwd = first_string(&value, &["cwd", "workDir"]).unwrap_or_else(|| {
        if work_dir.is_empty() {
            project_cwd.to_string()
        } else {
            work_dir.to_string()
        }
    });
    Some(SessionRow {
        tool_id: ToolId::Kimi,
        id: id.to_string(),
        title,
        cwd,
        updated_at: updated,
        rename_kind: RenameKind::Native,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn lists_index_and_renames_state() {
        let dir = tempdir().unwrap();
        std::env::set_var("KIMI_CODE_HOME", dir.path());
        let session_dir = dir
            .path()
            .join("sessions")
            .join("wd_tools_portal_6e6c9ed76d76")
            .join("session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("state.json"),
            r#"{"id":"session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d","title":"自动标题","isCustomTitle":false,"lastPrompt":"看一下门户","updatedAt":1786416604399,"cwd":"D:/idea_jidian_projects/aitools/tools_portal"}"#,
        )
        .unwrap();
        let mut index = fs::File::create(dir.path().join("session_index.jsonl")).unwrap();
        writeln!(
            index,
            r#"{{"sessionId":"session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d","sessionDir":"{}","workDir":"D:/idea_jidian_projects/aitools/tools_portal"}}"#,
            session_dir.to_string_lossy().replace('\\', "/")
        )
        .unwrap();
        writeln!(
            index,
            r#"{{"sessionId":"session_other","sessionDir":"C:/tmp/other","workDir":"C:/Users/PS"}}"#
        )
        .unwrap();

        let rows = list_sessions(r"D:\idea_jidian_projects\aitools\tools_portal").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "自动标题");

        rename_session("session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d", "门户发版").unwrap();
        let rows = list_sessions("d:/idea_jidian_projects/aitools/tools_portal").unwrap();
        assert_eq!(rows[0].title, "门户发版");
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(session_dir.join("state.json")).unwrap()).unwrap();
        assert_eq!(saved["isCustomTitle"], true);
        assert_eq!(saved["custom_title"], "门户发版");

        delete_session("session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d").unwrap();
        assert!(list_sessions("d:/idea_jidian_projects/aitools/tools_portal")
            .unwrap()
            .is_empty());
        assert!(!session_dir.exists());
        let index_text = fs::read_to_string(dir.path().join("session_index.jsonl")).unwrap();
        assert!(!index_text.contains("session_7fdf43c0-aefc-4427-a355-f6194f3a6e4d"));
        assert!(index_text.contains("session_other"));
        std::env::remove_var("KIMI_CODE_HOME");
    }

    #[test]
    fn lists_real_home_without_panic() {
        if std::env::var_os("KIMI_CODE_HOME").is_some() {
            return;
        }
        let rows = list_sessions(r"D:\idea_jidian_projects\aitools\tools_portal").unwrap();
        assert!(
            rows.iter().all(|row| row.tool_id == ToolId::Kimi),
            "adapter should only return kimi rows"
        );
    }
}
