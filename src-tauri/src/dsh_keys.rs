use crate::dsh_credentials::{self, DshKeyStatus};
use crate::secret_box::{self, SealedFile};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

pub const LIVE_KEY_ID: &str = "__live__";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshKeyMeta {
    pub id: String,
    pub name: String,
    pub masked: String,
    pub updated_at: String,
    pub active: bool,
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshKeyBundle {
    pub status: DshKeyStatus,
    pub keys: Vec<DshKeyMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vault {
    version: u32,
    active_id: Option<String>,
    keys: Vec<StoredKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKey {
    id: String,
    name: String,
    secret: String,
    updated_at: String,
}

pub fn active_secret(app: &AppHandle) -> Option<String> {
    let vault = load_vault(app).ok()?;
    let id = vault.active_id.as_ref()?;
    vault
        .keys
        .iter()
        .find(|item| item.id == *id)
        .map(|item| item.secret.clone())
}

pub fn list(app: &AppHandle, project_dir: Option<&Path>) -> Result<DshKeyBundle, String> {
    let status = dsh_credentials::live_status(project_dir);
    Ok(list_from(load_vault(app), status))
}

pub fn add(app: &AppHandle, name: &str, secret: &str, project_dir: Option<&Path>) -> Result<DshKeyBundle, String> {
    let name = normalize_name(name)?;
    let secret = validate_secret(secret)?;
    let mut vault = vault_or_empty(load_vault(app));
    let id = Uuid::new_v4().to_string();
    vault.keys.push(StoredKey {
        id: id.clone(),
        name,
        secret: secret.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    });
    vault.active_id = Some(id);
    save_vault(app, &vault)?;
    apply_active(&vault)?;
    Ok(bundle(vault, project_dir))
}

pub fn switch_to(app: &AppHandle, id: &str, project_dir: Option<&Path>) -> Result<DshKeyBundle, String> {
    let mut vault = load_vault(app)?;
    if !vault.keys.iter().any(|item| item.id == id) {
        return Err("没有这个 DeepSeek Key。".into());
    }
    vault.active_id = Some(id.to_string());
    save_vault(app, &vault)?;
    apply_active(&vault)?;
    Ok(bundle(vault, project_dir))
}

pub fn delete(app: &AppHandle, id: &str, project_dir: Option<&Path>) -> Result<DshKeyBundle, String> {
    let mut vault = load_vault(app)?;
    let was_active = vault.active_id.as_deref() == Some(id);
    let before = vault.keys.len();
    vault.keys.retain(|item| item.id != id);
    if vault.keys.len() == before {
        return Err("没有这个 DeepSeek Key。".into());
    }
    if was_active {
        vault.active_id = vault.keys.first().map(|item| item.id.clone());
    }
    save_vault(app, &vault)?;
    apply_active(&vault)?;
    Ok(bundle(vault, project_dir))
}

pub fn rename(app: &AppHandle, id: &str, name: &str, project_dir: Option<&Path>) -> Result<DshKeyBundle, String> {
    let name = normalize_name(name)?;
    let mut vault = load_vault(app)?;
    let item = vault
        .keys
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| "没有这个 DeepSeek Key。".to_string())?;
    item.name = name;
    item.updated_at = Utc::now().to_rfc3339();
    save_vault(app, &vault)?;
    Ok(bundle(vault, project_dir))
}

fn bundle(vault: Vault, project_dir: Option<&Path>) -> DshKeyBundle {
    list_from(Ok(vault), dsh_credentials::live_status(project_dir))
}

fn empty_vault() -> Vault {
    Vault {
        version: 1,
        active_id: None,
        keys: Vec::new(),
    }
}

fn vault_or_empty(result: Result<Vault, String>) -> Vault {
    result.unwrap_or_else(|_| empty_vault())
}

fn list_from(vault: Result<Vault, String>, status: DshKeyStatus) -> DshKeyBundle {
    let (vault, vault_error) = match vault {
        Ok(vault) => (vault, None),
        Err(err) => (empty_vault(), Some(err)),
    };
    let active = vault.active_id.clone();
    let mut keys: Vec<DshKeyMeta> = vault
        .keys
        .into_iter()
        .map(|item| DshKeyMeta {
            active: active.as_deref() == Some(item.id.as_str()),
            masked: dsh_credentials::mask_secret(&item.secret),
            id: item.id,
            name: item.name,
            updated_at: item.updated_at,
            managed: true,
        })
        .collect();
    if keys.is_empty() {
        if let Some(live) = live_row(&status) {
            keys.push(live);
        }
    }
    DshKeyBundle {
        status,
        keys,
        vault_error,
    }
}

fn live_row(status: &DshKeyStatus) -> Option<DshKeyMeta> {
    if !status.configured || status.masked.is_empty() {
        return None;
    }
    Some(DshKeyMeta {
        id: LIVE_KEY_ID.into(),
        name: "当前正在使用".into(),
        masked: status.masked.clone(),
        updated_at: String::new(),
        active: true,
        managed: false,
    })
}

fn apply_active(vault: &Vault) -> Result<(), String> {
    let home = dirs::home_dir();
    let dsh_home = dsh_credentials::resolve_dsh_home(
        std::env::var(dsh_credentials::DSH_HOME_ENV).ok().as_deref(),
        home.as_deref(),
    );
    match vault
        .active_id
        .as_ref()
        .and_then(|id| vault.keys.iter().find(|item| item.id == *id))
    {
        Some(item) => {
            dsh_credentials::set_key(&dsh_home, &item.secret)?;
        }
        None => {
            dsh_credentials::unset_key(&dsh_home)?;
        }
    }
    Ok(())
}

fn normalize_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请填写 Key 的名称，例如 工作号。".into());
    }
    if name.chars().count() > 40 {
        return Err("名称太长，最多 40 个字。".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("名称不能包含控制字符。".into());
    }
    Ok(name.to_string())
}

fn validate_secret(secret: &str) -> Result<&str, String> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("API Key 不能为空。".into());
    }
    if secret.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("API Key 不能包含空格或控制字符。".into());
    }
    if secret.len() > 512 {
        return Err("API Key 过长，请检查是否粘贴了多余内容。".into());
    }
    Ok(secret)
}

fn load_vault(app: &AppHandle) -> Result<Vault, String> {
    let path = vault_path(app)?;
    if !path.exists() {
        return Ok(empty_vault());
    }
    let sealed: SealedFile =
        serde_json::from_str(&fs::read_to_string(&path).map_err(|err| format!("无法读取保险箱：{err}"))?)
            .map_err(|_| "保险箱文件损坏。".to_string())?;
    let wrapped = sealed
        .wrapped_key
        .as_deref()
        .map(secret_box::decode)
        .transpose()?;
    let master = secret_box::unwrap_master(&sealed.protect, wrapped.as_deref(), &master_path(app)?)?;
    let nonce = secret_box::decode(&sealed.nonce)?;
    let ciphertext = secret_box::decode(&sealed.ciphertext)?;
    let plain = secret_box::open(&master, &nonce, &ciphertext)?;
    serde_json::from_slice(&plain).map_err(|_| "保险箱内容无法解析。".to_string())
}

fn save_vault(app: &AppHandle, vault: &Vault) -> Result<(), String> {
    let path = vault_path(app)?;
    let master = resolve_or_create_master(app)?;
    let (protect, wrapped) = secret_box::wrap_master(&master)?;
    let json = serde_json::to_vec(vault).map_err(|_| "无法序列化保险箱。".to_string())?;
    let (nonce, ciphertext) = secret_box::seal(&master, &json)?;
    let file = SealedFile {
        version: 1,
        protect,
        wrapped_key: wrapped.map(|bytes| secret_box::encode(&bytes)),
        nonce: secret_box::encode(&nonce),
        ciphertext: secret_box::encode(&ciphertext),
    };
    let text = serde_json::to_string(&file).map_err(|_| "无法写入保险箱。".to_string())?;
    fs::write(&path, text).map_err(|err| format!("无法保存保险箱：{err}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn resolve_or_create_master(app: &AppHandle) -> Result<[u8; secret_box::MASTER_LEN], String> {
    let path = vault_path(app)?;
    let existing = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    } else {
        None
    };
    master_for_save(existing.as_ref(), &master_path(app)?)
}

fn master_for_save(
    existing: Option<&SealedFile>,
    master_file: &Path,
) -> Result<[u8; secret_box::MASTER_LEN], String> {
    if let Some(sealed) = existing {
        if let Ok(wrapped) = sealed
            .wrapped_key
            .as_deref()
            .map(secret_box::decode)
            .transpose()
        {
            if let Ok(master) = secret_box::unwrap_master(&sealed.protect, wrapped.as_deref(), master_file)
            {
                return Ok(master);
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = master_file;
        return secret_box::random_master();
    }
    #[cfg(not(windows))]
    {
        secret_box::ensure_master_file(master_file)
    }
}

fn vault_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("无法定位应用数据目录：{err}"))?
        .join("dsh-keys");
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建 Key 目录：{err}"))?;
    Ok(dir)
}

fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(vault_dir(app)?.join("vault.json"))
}

fn master_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(vault_dir(app)?.join("master.key"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vault() -> Vault {
        Vault {
            version: 1,
            active_id: Some("a".into()),
            keys: vec![
                StoredKey {
                    id: "a".into(),
                    name: "工作号".into(),
                    secret: "sk-work-abcdefghijk".into(),
                    updated_at: "t1".into(),
                },
                StoredKey {
                    id: "b".into(),
                    name: "个人号".into(),
                    secret: "sk-home-abcdefghijk".into(),
                    updated_at: "t2".into(),
                },
            ],
        }
    }

    #[test]
    fn meta_never_includes_secret() {
        let bundle = bundle(sample_vault(), None);
        let json = serde_json::to_string(&bundle).unwrap();
        assert!(!json.contains("sk-work-abcdefghijk"));
        assert!(!json.contains("sk-home-abcdefghijk"));
        assert_eq!(bundle.keys.len(), 2);
        assert!(bundle.keys[0].active);
        assert!(!bundle.keys[1].active);
        assert_eq!(bundle.keys[0].masked, dsh_credentials::mask_secret("sk-work-abcdefghijk"));
    }

    #[test]
    fn sealed_file_has_no_plaintext_secret() {
        let master = secret_box::random_master().unwrap();
        let json = serde_json::to_vec(&sample_vault()).unwrap();
        let (nonce, ct) = secret_box::seal(&master, &json).unwrap();
        let file = SealedFile {
            version: 1,
            protect: "dpapi".into(),
            wrapped_key: Some(secret_box::encode(&[7_u8; 32])),
            nonce: secret_box::encode(&nonce),
            ciphertext: secret_box::encode(&ct),
        };
        let text = serde_json::to_string(&file).unwrap();
        assert!(!text.contains("sk-work"));
        assert!(!text.contains("sk-home"));
        assert!(!text.contains("工作号"));
        assert!(!text.contains("个人号"));
    }

    #[test]
    fn name_and_secret_validation() {
        assert!(normalize_name("").is_err());
        assert!(normalize_name("工作号").is_ok());
        assert!(validate_secret("").is_err());
        assert!(validate_secret("sk x").is_err());
        assert!(validate_secret("sk-ok").is_ok());
    }

    fn file_status(masked: &str) -> DshKeyStatus {
        DshKeyStatus {
            configured: !masked.is_empty(),
            writable: true,
            source: if masked.is_empty() { "none".into() } else { "file".into() },
            masked: masked.into(),
            dsh_home: "/tmp/.dsh".into(),
            credentials_path: "/tmp/.dsh/.credentials.yaml".into(),
            env_blocks: false,
        }
    }

    #[test]
    fn vault_error_still_shows_live_harness_key() {
        let err = "无法用 Windows 用户凭据解密 Key。请确认是同一台电脑、同一个 Windows 用户。";
        let bundle = list_from(Err(err.into()), file_status("sk-a…mnop"));
        assert_eq!(bundle.keys.len(), 1);
        assert_eq!(bundle.keys[0].id, LIVE_KEY_ID);
        assert_eq!(bundle.keys[0].name, "当前正在使用");
        assert_eq!(bundle.keys[0].masked, "sk-a…mnop");
        assert!(bundle.keys[0].active);
        assert!(!bundle.keys[0].managed);
        assert_eq!(bundle.vault_error.as_deref(), Some(err));
        assert!(bundle.status.configured);
    }

    #[test]
    fn vault_error_without_live_key_is_empty_list_not_failure() {
        let bundle = list_from(Err("decrypt fail".into()), file_status(""));
        assert!(bundle.keys.is_empty());
        assert_eq!(bundle.vault_error.as_deref(), Some("decrypt fail"));
        assert!(!bundle.status.configured);
    }

    #[test]
    fn empty_vault_with_live_key_shows_harness_row() {
        let bundle = list_from(Ok(empty_vault()), file_status("sk-a…mnop"));
        assert_eq!(bundle.keys.len(), 1);
        assert_eq!(bundle.keys[0].id, LIVE_KEY_ID);
        assert!(!bundle.keys[0].managed);
        assert!(bundle.vault_error.is_none());
    }

    #[test]
    fn vault_keys_are_managed_and_hide_live_row() {
        let bundle = list_from(Ok(sample_vault()), file_status("sk-a…mnop"));
        assert_eq!(bundle.keys.len(), 2);
        assert!(bundle.keys.iter().all(|item| item.managed));
        assert!(bundle.vault_error.is_none());
        assert!(bundle.keys[0].active);
    }

    #[test]
    fn unreadable_vault_becomes_empty_for_write() {
        let vault = vault_or_empty(Err("无法用 Windows 用户凭据解密 Key。".into()));
        assert!(vault.keys.is_empty());
        assert!(vault.active_id.is_none());
    }

    #[test]
    fn fresh_master_when_existing_sealed_cannot_open() {
        let sealed = SealedFile {
            version: 1,
            protect: "file".into(),
            wrapped_key: None,
            nonce: secret_box::encode(&[0; 12]),
            ciphertext: secret_box::encode(&[1, 2, 3]),
        };
        let dir = tempfile::tempdir().unwrap();
        let master_file = dir.path().join("missing.key");
        let master = master_for_save(Some(&sealed), &master_file).unwrap();
        assert_eq!(master.len(), secret_box::MASTER_LEN);
    }
}
