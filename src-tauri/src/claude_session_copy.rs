//! Copy one conversation into another account without copying authentication.
use chrono::Utc;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use uuid::Uuid;

pub fn copy_session(
    source_home: &Path,
    target_home: &Path,
    cwd: &str,
    session_id: &str,
    title: &str,
) -> Result<String, String> {
    Uuid::parse_str(session_id).map_err(|_| "会话编号不合法。")?;
    let source = crate::tools::claude::find_session_file_in(source_home, cwd, session_id)
        .ok_or("此会话的记录尚未保存，请稍后重试。")?;
    let source_root = source_home.canonicalize().map_err(|_| "原账号目录不可用。")?;
    let target_root = target_home.canonicalize().map_err(|_| "目标账号目录不可用。")?;
    if source_root == target_root {
        return Err("请选择另一个账号；原账号可直接打开此会话。".into());
    }
    if !source.canonicalize().is_ok_and(|p| p.starts_with(&source_root)) {
        return Err("会话文件不在原账号目录内，无法复制。".into());
    }
    let new_id = Uuid::new_v4().to_string();
    let project = crate::tools::claude::encode_project_dir(cwd);
    let target_project = target_root.join("projects").join(project);
    create_scoped_dir(&target_project, &target_root)?;
    let temporary = target_project.join(format!(".{new_id}.tmp"));
    let destination = target_project.join(format!("{new_id}.jsonl"));
    let mut created_dirs = Vec::new();
    let result = (|| {
        let input = File::open(&source).map_err(|_| "无法读取原会话。")?;
        // Snapshot the existing prefix, rather than following a running session
        // indefinitely. An incomplete last record is rejected, never discarded.
        let length = input.metadata().map_err(|_| "无法读取原会话状态。")?.len();
        let output = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
            .map_err(|_| "无法创建续接会话，请检查目标目录权限。")?;
        rewrite_history(
            BufReader::new(input.take(length)),
            BufWriter::new(output),
            &new_id,
            session_id,
            title,
        )?;
        let sidecars = [
            (source.parent().unwrap().join(session_id), target_project.join(&new_id)),
            (source_home.join("file-history").join(session_id), target_root.join("file-history").join(&new_id)),
        ];
        for (from, to) in sidecars {
            if from.exists() {
                let parent = to.parent().ok_or("目标会话路径不合法。")?;
                create_scoped_dir(parent, &target_root)?;
                fs::create_dir(&to).map_err(|_| "无法创建续接会话附件目录。")?;
                created_dirs.push(to.clone());
                copy_tree(&from, &to, &source_root, 0)?;
            }
        }
        if destination.exists() {
            return Err("续接会话已存在，请重试。".into());
        }
        fs::rename(&temporary, &destination).map_err(|_| "无法保存续接会话。")?;
        Ok(new_id.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        for path in created_dirs {
            // Only remove directories created by this attempt, after verifying
            // their resolved absolute location and generated session name.
            if path.file_name().is_some_and(|n| n == new_id.as_str())
                && path.canonicalize().is_ok_and(|p| p.starts_with(&target_root)) {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
    result
}

fn create_scoped_dir(path: &Path, root: &Path) -> Result<(), String> {
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or("目标会话路径不合法。")?;
    }
    if !ancestor.canonicalize().is_ok_and(|p| p.starts_with(root)) {
        return Err("目标会话目录超出账号目录，无法复制。".into());
    }
    fs::create_dir_all(path).map_err(|_| "无法创建目标会话目录。")?;
    if !path.canonicalize().is_ok_and(|p| p.starts_with(root)) {
        return Err("目标会话目录超出账号目录，无法复制。".into());
    }
    Ok(())
}

fn rewrite_history(
    input: impl BufRead,
    mut output: impl Write,
    new_id: &str,
    source_id: &str,
    title: &str,
) -> Result<(), String> {
    let mut records = 0;
    for line in input.split(b'\n') {
        let line = line.map_err(|_| "读取原会话失败。")?;
        if line.iter().all(u8::is_ascii_whitespace) { continue; }
        let mut record: Value = serde_json::from_slice(&line)
            .map_err(|_| "原会话记录尚未写完或格式损坏，请稍后重试。")?;
        {
            let object = record.as_object_mut().ok_or("原会话记录格式不受支持。")?;
            if object.contains_key("sessionId") {
                object.insert("sessionId".into(), json!(new_id));
            }
        }
        strip_assistant_usage(&mut record);
        serde_json::to_writer(&mut output, &record).map_err(|_| "保存续接会话失败。")?;
        output.write_all(b"\n").map_err(|_| "保存续接会话失败。")?;
        records += 1;
    }
    if records == 0 { return Err("原会话还没有可续接的记录。".into()); }
    serde_json::to_writer(
        &mut output,
        &json!({
            "type": "agent-dock-copy-boundary",
            "sessionId": new_id,
            "sourceSessionId": source_id,
            "copiedAt": Utc::now().to_rfc3339()
        }),
    )
    .map_err(|_| "保存续接会话失败。")?;
    output.write_all(b"\n").map_err(|_| "保存续接会话失败。")?;
    serde_json::to_writer(&mut output, &json!({"type": "custom-title", "sessionId": new_id, "customTitle": title}))
        .map_err(|_| "保存续接会话名称失败。")?;
    output.write_all(b"\n").and_then(|_| output.flush()).map_err(|_| "保存续接会话失败。".to_string())
}

fn copy_tree(from: &Path, to: &Path, source_root: &Path, depth: usize) -> Result<(), String> {
    let metadata = fs::symlink_metadata(from).map_err(|_| "无法读取会话附件。")?;
    #[cfg(windows)]
    let reparse = { use std::os::windows::fs::MetadataExt; metadata.file_attributes() & 0x400 != 0 };
    #[cfg(not(windows))]
    let reparse = false;
    if depth > 32 || metadata.file_type().is_symlink() || reparse
        || !from.canonicalize().is_ok_and(|p| p.starts_with(source_root)) {
        return Err("会话附件包含外部链接或目录层级过深，无法复制。".into());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(from).map_err(|_| "无法读取会话附件目录。")? {
            let entry = entry.map_err(|_| "无法读取会话附件。")?;
            let target = to.join(entry.file_name());
            let kind = entry.file_type().map_err(|_| "无法读取会话附件。")?;
            if kind.is_dir() { fs::create_dir(&target).map_err(|_| "无法创建会话附件目录。")?; }
            copy_tree(&entry.path(), &target, source_root, depth + 1)?;
        }
    } else if metadata.is_file() {
        if to.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            copy_jsonl_stripping_usage(from, to)?;
        } else {
            let mut output = fs::OpenOptions::new().write(true).create_new(true).open(to)
                .map_err(|_| "无法保存会话附件。")?;
            std::io::copy(&mut File::open(from).map_err(|_| "无法读取会话附件。")?, &mut output)
                .map_err(|_| "无法复制会话附件。")?;
        }
    } else { return Err("会话附件类型不受支持。".into()); }
    Ok(())
}

fn strip_assistant_usage(record: &mut Value) {
    if record.get("type").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    if let Some(message) = record.get_mut("message").and_then(Value::as_object_mut) {
        message.remove("usage");
    }
}

fn copy_jsonl_stripping_usage(from: &Path, to: &Path) -> Result<(), String> {
    let input = File::open(from).map_err(|_| "无法读取会话附件。")?;
    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(to)
        .map_err(|_| "无法保存会话附件。")?;
    for line in BufReader::new(input).split(b'\n') {
        let line = line.map_err(|_| "无法读取会话附件。")?;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(&line) {
            Ok(mut record) => {
                strip_assistant_usage(&mut record);
                serde_json::to_writer(&mut output, &record).map_err(|_| "无法保存会话附件。")?;
                output.write_all(b"\n").map_err(|_| "无法保存会话附件。")?;
            }
            Err(_) => {
                output.write_all(&line).map_err(|_| "无法保存会话附件。")?;
                output.write_all(b"\n").map_err(|_| "无法保存会话附件。")?;
            }
        }
    }
    output.flush().map_err(|_| "无法保存会话附件。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, String) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        let project = source.join("projects").join(crate::tools::claude::encode_project_dir("D:/project"));
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(&target).unwrap();
        let id = Uuid::new_v4().to_string();
        let file = project.join(format!("{id}.jsonl"));
        (root, source, target, file, id)
    }

    #[test]
    fn copies_only_history_with_new_id_and_preserves_original_and_target_credentials() {
        let (_root, source, target, file, id) = fixture();
        let record = json!({"type":"user", "sessionId":id,"uuid":"message-1","parentUuid":null,"message":{"content": format!("Keep literal ID {id}")}});
        fs::write(&file, format!("{record}\n")).unwrap();
        fs::write(source.join(".credentials.json"), "source-secret").unwrap();
        fs::write(target.join(".credentials.json"), "target-secret").unwrap();
        let attachments = file.parent().unwrap().join(&id).join("tool-results");
        fs::create_dir_all(&attachments).unwrap();
        fs::write(attachments.join("result.txt"), "tool result").unwrap();
        let rewind = source.join("file-history").join(&id);
        fs::create_dir_all(&rewind).unwrap();
        fs::write(rewind.join("backup"), "original source file").unwrap();
        let original = fs::read(&file).unwrap();
        let next = copy_session(&source, &target, "D:/project", &id, "继续任务").unwrap();
        assert_ne!(next, id);
        let copied = crate::tools::claude::find_session_file_in(&target, "D:/project", &next).unwrap();
        let rows: Vec<Value> = fs::read_to_string(&copied).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect();
        assert_eq!(rows[0]["sessionId"], next);
        assert_eq!(rows[0]["uuid"], "message-1");
        assert_eq!(rows[0]["message"], record["message"]);
        let boundary = rows.iter().find(|row| row["type"] == "agent-dock-copy-boundary")
            .expect("copy writes an account-copy boundary");
        assert_eq!(boundary["sessionId"], next);
        assert_eq!(boundary["sourceSessionId"], id);
        assert!(boundary["copiedAt"].as_str().is_some_and(|s| !s.is_empty()));
        let boundary_at = rows.iter().position(|row| row["type"] == "agent-dock-copy-boundary").unwrap();
        let title_at = rows.iter().position(|row| row.get("customTitle").is_some()).unwrap();
        assert!(boundary_at < title_at);
        assert_eq!(rows.last().unwrap()["customTitle"], "继续任务");
        assert_eq!(fs::read(&file).unwrap(), original);
        assert_eq!(fs::read_to_string(target.join(".credentials.json")).unwrap(), "target-secret");
        assert_eq!(fs::read_to_string(copied.parent().unwrap().join(&next).join("tool-results/result.txt")).unwrap(), "tool result");
        assert_eq!(fs::read_to_string(target.join("file-history").join(&next).join("backup")).unwrap(), "original source file");
    }

    #[test]
    fn copy_strips_assistant_usage_from_history_and_sidecar_jsonl() {
        let (_root, source, target, file, id) = fixture();
        let usage = json!({
            "type": "assistant",
            "sessionId": id,
            "uuid": "asst-1",
            "timestamp": "2026-01-17T10:00:00Z",
            "message": {
                "model": "claude-opus-4",
                "usage": {"input_tokens": 99, "output_tokens": 2}
            }
        });
        fs::write(&file, format!("{usage}\n")).unwrap();
        let sidecar = file.parent().unwrap().join(&id).join("subagents");
        fs::create_dir_all(&sidecar).unwrap();
        fs::write(sidecar.join("agent-1.jsonl"), format!("{usage}\n")).unwrap();
        let next = copy_session(&source, &target, "D:/project", &id, "继续任务").unwrap();
        let copied = crate::tools::claude::find_session_file_in(&target, "D:/project", &next).unwrap();
        let main: Vec<Value> = fs::read_to_string(&copied).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect();
        let assistant = main.iter().find(|row| row["type"] == "assistant").unwrap();
        assert!(assistant["message"].get("usage").is_none());
        let copied_sidecar = copied.parent().unwrap().join(&next).join("subagents").join("agent-1.jsonl");
        let side: Value = serde_json::from_str(&fs::read_to_string(copied_sidecar).unwrap()).unwrap();
        assert!(side["message"].get("usage").is_none());
        assert_eq!(side["type"], "assistant");
    }

    #[test]
    fn incomplete_history_is_not_published_and_traversal_is_rejected() {
        let (_root, source, target, file, id) = fixture();
        fs::write(&file, b"{\"type\":\"user\"").unwrap();
        assert!(copy_session(&source, &target, "D:/project", &id, "任务").is_err());
        let project = target.join("projects").join(crate::tools::claude::encode_project_dir("D:/project"));
        assert_eq!(fs::read_dir(project).unwrap().count(), 0);
        assert!(copy_session(&source, &target, "D:/project", "../credentials", "任务").is_err());
    }
}
