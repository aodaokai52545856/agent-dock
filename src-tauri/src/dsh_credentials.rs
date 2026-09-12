use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const KEY_NAME: &str = "DEEPSEEK_API_KEY";
pub const DSH_HOME_ENV: &str = "DSH_HOME";
pub const DSH_HOME_DIR: &str = ".dsh";
pub const CREDENTIALS_FILE: &str = ".credentials.yaml";
pub const USER_ENV_FILE: &str = ".env";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshKeyStatus {
    pub configured: bool,
    pub writable: bool,
    pub source: String,
    pub masked: String,
    pub dsh_home: String,
    pub credentials_path: String,
    pub env_blocks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialsDoc {
    version: i64,
    #[serde(default)]
    refs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    records: Option<serde_yaml::Value>,
}

pub fn resolve_dsh_home(dsh_home_env: Option<&str>, user_home: Option<&Path>) -> PathBuf {
    if let Some(raw) = dsh_home_env.map(str::trim).filter(|item| !item.is_empty()) {
        return expand_home(raw, user_home);
    }
    match user_home {
        Some(home) => home.join(DSH_HOME_DIR),
        None => PathBuf::from(DSH_HOME_DIR),
    }
}

pub fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 8 {
        return "••••".into();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{head}…{tail}")
}

pub fn parse_dotenv(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        out.insert(key.to_string(), value);
    }
    out
}

pub fn describe(
    dsh_home: &Path,
    process_key: Option<&str>,
    project_dir: Option<&Path>,
) -> DshKeyStatus {
    let credentials_path = dsh_home.join(CREDENTIALS_FILE);
    let env_value = process_key.map(str::trim).filter(|item| !item.is_empty());
    let file_value = read_file_key(&credentials_path).ok().flatten();
    let project_value = project_dir.and_then(|dir| read_dotenv_key(&dir.join(".env")));
    let user_env_value = read_dotenv_key(&dsh_home.join(USER_ENV_FILE));

    let (source, value) = if let Some(value) = env_value {
        ("env", value.to_string())
    } else if let Some(value) = file_value {
        ("file", value)
    } else if let Some(value) = project_value {
        ("project-env", value)
    } else if let Some(value) = user_env_value {
        ("user-env", value)
    } else {
        ("none", String::new())
    };

    DshKeyStatus {
        configured: source != "none",
        writable: true,
        source: source.into(),
        masked: mask_secret(&value),
        dsh_home: dsh_home.display().to_string(),
        credentials_path: credentials_path.display().to_string(),
        env_blocks: source == "env",
    }
}

pub fn set_key(dsh_home: &Path, key: &str) -> Result<DshKeyStatus, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API Key 不能为空。要删除请用清除。".into());
    }
    if key.chars().any(|c| c.is_control()) {
        return Err("API Key 不能包含控制字符。".into());
    }
    if key.len() > 512 {
        return Err("API Key 过长，请检查是否粘贴了多余内容。".into());
    }
    let path = dsh_home.join(CREDENTIALS_FILE);
    let mut doc = load_or_default(&path)?;
    doc.refs.insert(KEY_NAME.into(), key.to_string());
    write_doc(&path, &doc)?;
    Ok(describe(dsh_home, std::env::var(KEY_NAME).ok().as_deref(), None))
}

pub fn unset_key(dsh_home: &Path) -> Result<DshKeyStatus, String> {
    let path = dsh_home.join(CREDENTIALS_FILE);
    if path.exists() {
        let mut doc = load_or_default(&path)?;
        doc.refs.remove(KEY_NAME);
        write_doc(&path, &doc)?;
    }
    Ok(describe(dsh_home, std::env::var(KEY_NAME).ok().as_deref(), None))
}

pub fn live_status(project_dir: Option<&Path>) -> DshKeyStatus {
    let home = dirs::home_dir();
    let dsh_home = resolve_dsh_home(std::env::var(DSH_HOME_ENV).ok().as_deref(), home.as_deref());
    describe(&dsh_home, std::env::var(KEY_NAME).ok().as_deref(), project_dir)
}

fn expand_home(raw: &str, user_home: Option<&Path>) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        if let Some(home) = user_home {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

fn read_dotenv_key(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    parse_dotenv(&text)
        .get(KEY_NAME)
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn read_file_key(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let doc = load_or_default(path)?;
    Ok(doc
        .refs
        .get(KEY_NAME)
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty()))
}

fn load_or_default(path: &Path) -> Result<CredentialsDoc, String> {
    if !path.exists() {
        return Ok(CredentialsDoc {
            version: 1,
            refs: BTreeMap::new(),
            records: None,
        });
    }
    let text = fs::read_to_string(path).map_err(|err| format!("无法读取凭据文件：{err}"))?;
    parse_doc(&text)
}

fn parse_doc(text: &str) -> Result<CredentialsDoc, String> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|_| "DeepSeek Harness 凭据文件无法解析。".to_string())?;
    if let Some(map) = value.as_mapping() {
        if map.get(serde_yaml::Value::from("version")).is_none()
            && map.get(serde_yaml::Value::from("refs")).is_none()
        {
            let mut refs = BTreeMap::new();
            for (key, item) in map {
                let Some(name) = key.as_str() else { continue };
                if let Some(secret) = item.as_str() {
                    refs.insert(name.to_string(), secret.to_string());
                }
            }
            return Ok(CredentialsDoc {
                version: 1,
                refs,
                records: None,
            });
        }
    }
    serde_yaml::from_str(text).map_err(|_| "DeepSeek Harness 凭据文件格式不正确。".to_string())
}

fn write_doc(path: &Path, doc: &CredentialsDoc) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建 Harness 目录：{err}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let text = serde_yaml::to_string(doc).map_err(|_| "无法生成凭据文件。".to_string())?;
    fs::write(path, text).map_err(|err| format!("无法写入凭据文件：{err}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_home_is_dot_dsh_under_user_home() {
        let home = PathBuf::from("/Users/demo");
        assert_eq!(resolve_dsh_home(None, Some(&home)), home.join(".dsh"));
    }

    #[test]
    fn dsh_home_env_wins() {
        let home = PathBuf::from("/Users/demo");
        assert_eq!(
            resolve_dsh_home(Some("~/work/dsh"), Some(&home)),
            home.join("work").join("dsh")
        );
        assert_eq!(
            resolve_dsh_home(Some("  "), Some(&home)),
            home.join(".dsh")
        );
    }

    #[test]
    fn mask_keeps_head_and_tail() {
        assert_eq!(mask_secret("sk-abcdefghijklmnop"), "sk-a…mnop");
        assert_eq!(mask_secret("short"), "••••");
        assert_eq!(mask_secret("  "), "");
    }

    #[test]
    fn dotenv_reads_quoted_and_plain() {
        let map = parse_dotenv("DEEPSEEK_API_KEY=sk-plain\n# c\nOTHER='x'\nDEEPSEEK_BASE_URL=\"https://api.deepseek.com\"\n");
        assert_eq!(map.get("DEEPSEEK_API_KEY").unwrap(), "sk-plain");
        assert_eq!(map.get("DEEPSEEK_BASE_URL").unwrap(), "https://api.deepseek.com");
    }

    #[test]
    fn env_wins_over_file_and_dotenv() {
        let root = tempfile::tempdir().unwrap();
        let dsh = root.path().join(".dsh");
        fs::create_dir_all(&dsh).unwrap();
        fs::write(dsh.join(".env"), "DEEPSEEK_API_KEY=from-user-env\n").unwrap();
        set_key(&dsh, "from-file").unwrap();
        let status = describe(&dsh, Some("from-process"), None);
        assert_eq!(status.source, "env");
        assert!(status.env_blocks);
        assert_eq!(status.masked, mask_secret("from-process"));
    }

    #[test]
    fn file_wins_over_home_env() {
        let root = tempfile::tempdir().unwrap();
        let dsh = root.path().join(".dsh");
        fs::create_dir_all(&dsh).unwrap();
        fs::write(dsh.join(".env"), "DEEPSEEK_API_KEY=from-user-env\n").unwrap();
        set_key(&dsh, "from-file").unwrap();
        let status = describe(&dsh, None, None);
        assert_eq!(status.source, "file");
        assert!(!status.env_blocks);
        assert_eq!(status.masked, mask_secret("from-file"));
    }

    #[test]
    fn set_preserves_other_refs_and_records() {
        let root = tempfile::tempdir().unwrap();
        let dsh = root.path().join(".dsh");
        fs::create_dir_all(&dsh).unwrap();
        let path = dsh.join(CREDENTIALS_FILE);
        fs::write(
            &path,
            "version: 1\nrefs:\n  OPENAI_API_KEY: sk-openai\nrecords:\n  llm-pi-ai/openai-codex:\n    kind: grant\n    payload:\n      type: oauth\n",
        )
        .unwrap();
        set_key(&dsh, "sk-deepseek").unwrap();
        let doc = load_or_default(&path).unwrap();
        assert_eq!(doc.refs.get("OPENAI_API_KEY").unwrap(), "sk-openai");
        assert_eq!(doc.refs.get(KEY_NAME).unwrap(), "sk-deepseek");
        assert!(doc.records.is_some());
    }

    #[test]
    fn unset_removes_only_deepseek_ref() {
        let root = tempfile::tempdir().unwrap();
        let dsh = root.path().join(".dsh");
        fs::create_dir_all(&dsh).unwrap();
        set_key(&dsh, "sk-deepseek").unwrap();
        unset_key(&dsh).unwrap();
        let doc = load_or_default(&dsh.join(CREDENTIALS_FILE)).unwrap();
        assert!(!doc.refs.contains_key(KEY_NAME));
    }

    #[test]
    fn migrates_flat_pre_release_layout() {
        let text = "DEEPSEEK_API_KEY: sk-flat\nOPENAI_API_KEY: sk-o\n";
        let doc = parse_doc(text).unwrap();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.refs.get(KEY_NAME).unwrap(), "sk-flat");
        assert_eq!(doc.refs.get("OPENAI_API_KEY").unwrap(), "sk-o");
    }
}
