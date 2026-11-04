//! OMP's native journals. Public ids include the profile so every existing
//! session API (including overlays and artifact keys) remains unambiguous.
use super::{file_mtime_millis, matches_cwd, truncate_title, RenameKind, SessionRow, ToolId};
use crate::{platform, state::AppSettings};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub agent_dir: PathBuf,
}
pub fn config_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(std::env::var_os("PI_CONFIG_DIR").unwrap_or_else(|| ".omp".into()))
}
pub fn agent_dir(profile: &str) -> PathBuf {
    if profile == "default" {
        std::env::var_os("PI_CODING_AGENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| config_root().join("agent"))
    } else {
        config_root().join("profiles").join(profile).join("agent")
    }
}
pub fn default_profile(settings: &AppSettings) -> String {
    if !settings.omp_default_profile.trim().is_empty() {
        return settings.omp_default_profile.trim().into();
    }
    std::env::var("OMP_PROFILE")
        .or_else(|_| std::env::var("PI_PROFILE"))
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "default".into())
}
pub fn validate_profile(profile: &str) -> Result<(), String> {
    if profile.is_empty()
        || profile == "."
        || profile == ".."
        || !profile
            .chars()
            .all(|c| c.is_alphanumeric() || "_-".contains(c))
    {
        return Err("OMP profile 名称无效".into());
    }
    Ok(())
}
pub fn profiles() -> Vec<Profile> {
    let mut names = vec!["default".to_string()];
    if let Ok(entries) = fs::read_dir(config_root().join("profiles")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() && validate_profile(&name).is_ok() {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|id| Profile {
            agent_dir: agent_dir(&id),
            id,
        })
        .collect()
}
pub fn public_id(profile: &str, id: &str) -> String {
    format!("omp@{}:{id}", URL_SAFE_NO_PAD.encode(profile))
}
pub fn split_id(id: &str) -> Result<(String, String), String> {
    let (p, s) = id
        .strip_prefix("omp@")
        .and_then(|x| x.split_once(':'))
        .ok_or("OMP 会话引用缺少 profile")?;
    let p = String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(p)
            .map_err(|_| "OMP profile 编码无效")?,
    )
    .map_err(|_| "OMP profile 编码无效")?;
    validate_profile(&p)?;
    if s.is_empty() {
        return Err("OMP 会话 ID 为空".into());
    }
    Ok((p, s.into()))
}
pub fn session_roots(profile: &str) -> Vec<PathBuf> {
    let mut roots = vec![agent_dir(profile).join("sessions")];
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        let mut p = PathBuf::from(data).join("omp");
        if profile != "default" {
            p = p.join("profiles").join(profile);
        }
        if p.is_dir() {
            roots.insert(0, p.join("sessions"));
        }
    }
    roots
}
pub fn journals(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(dirs) = fs::read_dir(root) {
        for d in dirs.flatten() {
            if !d.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            if let Ok(files) = fs::read_dir(d.path()) {
                for f in files.flatten() {
                    if f.file_type().is_ok_and(|t| t.is_file())
                        && f.path().extension().is_some_and(|e| e == "jsonl")
                    {
                        out.push(f.path());
                    }
                }
            }
        }
    }
    out
}
pub fn entries(path: &Path) -> Vec<Value> {
    let bytes = fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
pub fn header(entries: &[Value]) -> Option<&Value> {
    entries.iter().find(|v| v["type"] == "session")
}
pub fn text(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return s.into();
    }
    content
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v["type"] == "text")
                .filter_map(|v| v["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
pub fn branch(entries: &[Value]) -> Vec<&Value> {
    let by_id: HashMap<&str, &Value> = entries
        .iter()
        .filter_map(|v| v["id"].as_str().map(|id| (id, v)))
        .collect();
    let Some(last) = entries.iter().rev().find(|v| {
        v["parentId"].is_string() || v["parentId"].is_null() && v.get("parentId").is_some()
    }) else {
        return entries.iter().filter(|v| v["type"] == "message").collect();
    };
    let mut next = last.get("activeLeafId").unwrap_or(&last["id"]).as_str();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    while let Some(id) = next {
        if !seen.insert(id) {
            break;
        }
        let Some(v) = by_id.get(id) else { break };
        out.push(*v);
        next = v["parentId"].as_str();
    }
    out.reverse();
    out
}
pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in profiles() {
        for root in session_roots(&p.id) {
            for path in journals(&root) {
                let es = entries(&path);
                let Some(h) = header(&es) else { continue };
                if !matches_cwd(h["cwd"].as_str().unwrap_or(""), cwd) {
                    continue;
                }
                let Some(id) = h["id"].as_str() else { continue };
                let id = public_id(&p.id, id);
                if !seen.insert(id.clone()) {
                    continue;
                }
                let title = es
                    .iter()
                    .find(|v| v["type"] == "title")
                    .and_then(|v| v["title"].as_str())
                    .or_else(|| h["title"].as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        es.iter()
                            .rev()
                            .find(|v| v["type"] == "session_info")
                            .and_then(|v| v["name"].as_str())
                            .map(str::to_string)
                    })
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| {
                        es.iter()
                            .find(|v| v["message"]["role"] == "user")
                            .map(|v| text(&v["message"]["content"]))
                            .unwrap_or_else(|| "OMP 会话".into())
                    });
                out.push(SessionRow {
                    profile_id: Some(p.id.clone()),
                    tool_id: ToolId::Omp,
                    id,
                    title: truncate_title(&title, 80),
                    cwd: cwd.into(),
                    updated_at: file_mtime_millis(&path),
                    rename_kind: RenameKind::Native,
                    account: Some(super::SessionAccount {
                        id: p.id.clone(),
                        name: p.id.clone(),
                    }),
                });
            }
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}
pub fn find_session_file(cwd: &str, public: &str) -> Option<PathBuf> {
    let (profile, id) = split_id(public).ok()?;
    for root in session_roots(&profile) {
        for file in journals(&root) {
            let es = entries(&file);
            if header(&es)
                .is_some_and(|h| h["id"] == id && matches_cwd(h["cwd"].as_str().unwrap_or(""), cwd))
            {
                return Some(file);
            }
        }
    }
    None
}
pub fn command(settings: &AppSettings, profile: &str) -> Result<Command, String> {
    validate_profile(profile)?;
    let exe = super::resolve_binary(ToolId::Omp, settings)?;
    let mut cmd = Command::new(exe);
    platform::apply_no_window(&mut cmd);
    cmd.args(["--profile", profile]);
    Ok(cmd)
}
/// Same session bucket convention as OMP's session-paths.ts. Passing the native
/// bucket explicitly creates a fresh session even when autoResume is enabled.
pub fn native_session_dir(cwd: &str, profile: &str) -> PathBuf {
    let canonical = |p: PathBuf| fs::canonicalize(&p).unwrap_or(p);
    let cwd = canonical(PathBuf::from(cwd));
    let encode = |s: &str| {
        s.chars()
            .map(|c| {
                if matches!(c, '/' | '\\' | ':') {
                    '-'
                } else {
                    c
                }
            })
            .collect::<String>()
    };
    let mut bucket = None;
    for (prefix, root) in [
        ("-tmp", Some(std::env::temp_dir())),
        ("-", dirs::home_dir()),
    ] {
        if let Some(root) = root {
            if let Ok(relative) = cwd.strip_prefix(canonical(root)) {
                let relative = encode(&relative.to_string_lossy());
                bucket = Some(if relative.is_empty() {
                    prefix.into()
                } else if prefix.ends_with('-') {
                    format!("{prefix}{relative}")
                } else {
                    format!("{prefix}-{relative}")
                });
                break;
            }
        }
    }
    let bucket = bucket.unwrap_or_else(|| {
        let path = cwd.to_string_lossy();
        let path = path.strip_prefix(r"\\?\").unwrap_or(&path);
        format!("--{}--", encode(path.trim_start_matches(['/', '\\'])))
    });
    let root = if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        let root = PathBuf::from(data).join("omp");
        if profile == "default" {
            root
        } else {
            root.join("profiles").join(profile)
        }
        .join("sessions")
    } else {
        agent_dir(profile).join("sessions")
    };
    root.join(bucket)
}
pub fn native_args(cwd: &str, session: Option<&str>, profile: &str) -> Result<Vec<String>, String> {
    validate_profile(profile)?;
    let mut args = vec!["--profile".into(), profile.into()];
    if let Some(session) = session {
        if split_id(session)?.0 != profile {
            return Err("OMP profile 与会话不匹配".into());
        }
        let path = find_session_file(cwd, session).ok_or("OMP 历史文件不存在")?;
        args.extend(["--resume".into(), path.to_string_lossy().into_owned()]);
    } else {
        args.extend([
            "--session-dir".into(),
            native_session_dir(cwd, profile)
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    Ok(args)
}
pub fn ensure_free(cwd: &str, id: &str, settings: &AppSettings) -> Result<PathBuf, String> {
    let path = find_session_file(cwd, id).ok_or("OMP 会话文件不存在")?;
    let (profile, sid) = split_id(id)?;
    let output = command(settings, &profile)?
        .args(["session", "lease", "--json", "--", &sid])
        .output()
        .map_err(|e| e.to_string())?;
    let value: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "无法检查 OMP 会话占用，操作已取消")?;
    if !output.status.success()
        || !value["held"].as_array().is_some_and(|a| a.is_empty())
        || !value["unknown"].as_array().is_some_and(|a| a.is_empty())
    {
        return Err("此 OMP 会话正在使用或占用状态未知，请先关闭后重试".into());
    }
    Ok(path)
}
pub fn delete_session(cwd: &str, id: &str, settings: &AppSettings) -> Result<(), String> {
    let path = ensure_free(cwd, id, settings)?;
    fs::remove_file(path).map_err(|e| e.to_string())
}
pub fn rename_session(
    cwd: &str,
    id: &str,
    title: &str,
    settings: &AppSettings,
) -> Result<(), String> {
    // Use OMP's writer so title slots, indexes and leases stay consistent.
    if crate::omp_rpc::rename_running(id, title)? {
        return Ok(());
    }
    let path = ensure_free(cwd, id, settings)?;
    let (profile, _) = split_id(id)?;
    let mut cmd = command(settings, &profile)?;
    cmd.current_dir(cwd)
        .args(["--mode", "rpc-ui", "--no-ui", "--resume"])
        .arg(path);
    let (client, _events) =
        omp_rpc::Client::spawn(cmd, Default::default()).map_err(|e| e.to_string())?;
    client
        .request_value(
            json!({"type":"set_session_name","name":title}),
            std::time::Duration::from_secs(30),
        )
        .map_err(|e| e.to_string())?;
    client.close().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_start_is_interactive_and_uses_the_profiles_native_history() {
        let cwd = tempfile::tempdir().unwrap();
        let args = native_args(cwd.path().to_str().unwrap(), None, "work").unwrap();
        assert_eq!(&args[..2], &["--profile", "work"]);
        assert_eq!(args[2], "--session-dir");
        assert!(!args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--mode" | "rpc-ui" | "--no-ui")));
        assert!(Path::new(&args[3]).starts_with(session_roots("work")[0].clone()));
        assert!(native_args(
            cwd.path().to_str().unwrap(),
            Some(&public_id("other", "session")),
            "work"
        )
        .unwrap_err()
        .contains("profile"));
    }
    #[test]
    fn profile_ids_round_trip_without_path_traversal() {
        let id = public_id("work", "uuid:part");
        assert_eq!(split_id(&id).unwrap(), ("work".into(), "uuid:part".into()));
        assert!(validate_profile("../bad").is_err());
    }
    #[test]
    fn only_current_branch_is_replayed() {
        let es = vec![
            json!({"id":"a","parentId":null}),
            json!({"id":"b","parentId":"a"}),
            json!({"id":"c","parentId":"a"}),
        ];
        assert_eq!(
            branch(&es)
                .iter()
                .map(|v| v["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["a", "c"]
        );
    }
    #[test]
    fn late_usage_keeps_the_captured_leaf() {
        let es = vec![
            json!({"id":"a","parentId":null}),
            json!({"id":"b","parentId":"a"}),
            json!({"id":"usage","parentId":"b","activeLeafId":"a"}),
        ];
        assert_eq!(
            branch(&es)
                .iter()
                .map(|v| v["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
    }
    #[test]
    fn an_incomplete_utf8_tail_does_not_hide_existing_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut content=b"{\"type\":\"title\",\"title\":\"Title\"}\n{\"type\":\"session\",\"id\":\"one\",\"cwd\":\"D:/project\"}\n".to_vec();
        content.extend_from_slice(&[0xe4, 0xb8]);
        fs::write(&path, content).unwrap();
        let es = entries(&path);
        assert_eq!(header(&es).unwrap()["id"], "one");
        assert_eq!(es[0]["title"], "Title");
    }
}
