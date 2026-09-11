use super::{
    RenameKind, SessionRow, ToolId, file_mtime_millis, first_string, matches_cwd, parse_time_value,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub fn grok_home() -> PathBuf {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".grok"))
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    let root = grok_home().join("sessions");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    let cwd_dirs = fs::read_dir(&root).map_err(|err| format!("无法读取 Grok session 目录：{err}"))?;
    for cwd_entry in cwd_dirs.flatten() {
        let cwd_path = cwd_entry.path();
        if !cwd_path.is_dir() {
            continue;
        }
        let session_dirs = match fs::read_dir(&cwd_path) {
            Ok(iter) => iter,
            Err(_) => continue,
        };
        for session_entry in session_dirs.flatten() {
            let session_dir = session_entry.path();
            if !session_dir.is_dir() {
                continue;
            }
            let summary_path = session_dir.join("summary.json");
            if !summary_path.is_file() {
                continue;
            }
            if let Some(row) = parse_summary(&summary_path, cwd) {
                rows.push(row);
            }
        }
    }
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(rows)
}

pub fn rename_session(cwd: &str, session_id: &str, title: &str) -> Result<(), String> {
    let path = find_summary(cwd, session_id).ok_or_else(|| {
        "没有找到这个 Grok session 的 summary.json，无法改名。请确认它仍在 ~/.grok/sessions 下。"
            .to_string()
    })?;
    let text = fs::read_to_string(&path).map_err(|err| format!("读取 summary.json 失败：{err}"))?;
    let mut value: Value =
        serde_json::from_str(&text).map_err(|err| format!("summary.json 不是合法 JSON：{err}"))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "summary.json 根节点必须是对象".to_string())?;
    obj.insert("title".into(), json!(title));
    obj.insert("title_is_manual".into(), json!(true));
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&value).unwrap())
        .map_err(|err| format!("写入标题失败：{err}"))?;
    fs::rename(&tmp, &path).map_err(|err| format!("保存标题失败：{err}"))?;
    Ok(())
}

pub fn delete_session(cwd: &str, session_id: &str) -> Result<(), String> {
    let summary = find_summary(cwd, session_id).ok_or_else(|| {
        "没有找到这个 Grok session，无法删除。请确认它仍在 ~/.grok/sessions 下。".to_string()
    })?;
    let dir = summary.parent().ok_or_else(|| "Grok session 目录无效".to_string())?;
    fs::remove_dir_all(dir).map_err(|err| format!("删除 Grok session 失败：{err}"))?;
    Ok(())
}

fn find_summary(cwd: &str, session_id: &str) -> Option<PathBuf> {
    let root = grok_home().join("sessions");
    let mut found = None;
    let cwd_dirs = fs::read_dir(root).ok()?;
    for cwd_entry in cwd_dirs.flatten() {
        let session_dir = cwd_entry.path().join(session_id);
        let summary = session_dir.join("summary.json");
        if !summary.is_file() {
            continue;
        }
        if let Some(row) = parse_summary(&summary, cwd) {
            if row.id == session_id {
                found = Some(summary);
                break;
            }
        }
    }
    found
}

fn parse_summary(path: &Path, project_cwd: &str) -> Option<SessionRow> {
    let text = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let info = value.get("info").cloned().unwrap_or(Value::Null);
    let session_cwd = first_string(&info, &["cwd"])
        .or_else(|| first_string(&value, &["cwd"]))
        .unwrap_or_default();
    if !session_cwd.is_empty() && !matches_cwd(&session_cwd, project_cwd) {
        return None;
    }
    let id = first_string(&info, &["id"])
        .or_else(|| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })?;
    let manual = value
        .get("title_is_manual")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let title = if manual {
        first_string(&value, &["title"])
    } else {
        None
    }
    .or_else(|| first_string(&value, &["title"]))
    .or_else(|| first_string(&value, &["generated_title"]))
    .or_else(|| first_string(&value, &["session_summary"]))
    .unwrap_or_else(|| id.clone());
    let updated = parse_time_value(value.get("updated_at").unwrap_or(&Value::Null))
        .or_else(|| parse_time_value(value.get("last_active_at").unwrap_or(&Value::Null)))
        .or_else(|| parse_time_value(value.get("created_at").unwrap_or(&Value::Null)))
        .unwrap_or_else(|| file_mtime_millis(path));
    Some(SessionRow {
        tool_id: ToolId::Grokbuild,
        id,
        title: title.lines().next().unwrap_or("").trim().to_string(),
        cwd: if session_cwd.is_empty() {
            project_cwd.to_string()
        } else {
            session_cwd
        },
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
    fn lists_and_renames_summary() {
        let dir = tempdir().unwrap();
        std::env::set_var("GROK_HOME", dir.path());
        let session = dir
            .path()
            .join("sessions")
            .join("D%3A%5Ccliproxy")
            .join("01a06c3d-79bf-7d71-b912-439d405d4fb5");
        fs::create_dir_all(&session).unwrap();
        let summary = session.join("summary.json");
        let mut file = fs::File::create(&summary).unwrap();
        write!(
            file,
            r#"{{
              "info": {{ "id": "01a06c3d-79bf-7d71-b912-439d405d4fb5", "cwd": "D:\\cliproxy" }},
              "session_summary": "old summary",
              "generated_title": "Add aliases",
              "updated_at": "2026-09-04T11:57:00Z"
            }}"#
        )
        .unwrap();

        let rows = list_sessions(r"D:\cliproxy").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Add aliases");

        rename_session(r"D:\cliproxy", "01a06c3d-79bf-7d71-b912-439d405d4fb5", "登录超时").unwrap();
        let rows = list_sessions(r"d:/cliproxy").unwrap();
        assert_eq!(rows[0].title, "登录超时");
        let saved: Value = serde_json::from_str(&fs::read_to_string(&summary).unwrap()).unwrap();
        assert_eq!(saved["title"], "登录超时");
        assert_eq!(saved["title_is_manual"], true);

        delete_session(r"D:\cliproxy", "01a06c3d-79bf-7d71-b912-439d405d4fb5").unwrap();
        assert!(list_sessions(r"D:\cliproxy").unwrap().is_empty());
        assert!(!session.exists());
        std::env::remove_var("GROK_HOME");
    }

    #[test]
    fn lists_real_home_without_panic() {
        if std::env::var_os("GROK_HOME").is_some() {
            return;
        }
        let rows = list_sessions(r"D:\cliproxy").unwrap();
        assert!(rows.iter().all(|row| row.tool_id == ToolId::Grokbuild));
    }
}
