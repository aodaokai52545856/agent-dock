use crate::platform;
use crate::proxy::proxy_env;
use crate::state::AppSettings;
use crate::tools::{self, ToolId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub user_id: String,
    pub updated_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokAccountList {
    pub logged_in: bool,
    pub current_email: String,
    pub current_name: String,
    pub accounts: Vec<GrokAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AccountIndex {
    accounts: Vec<AccountMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountMeta {
    id: String,
    name: String,
    email: String,
    user_id: String,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct Identity {
    email: String,
    user_id: String,
    display: String,
}

pub fn list_accounts(app: &AppHandle) -> Result<GrokAccountList, String> {
    persist_live(app)?;
    let current = read_live_identity();
    let index = load_index(app)?;
    let current_id = current
        .as_ref()
        .and_then(|id| find_meta(&index, id).map(|item| item.id.clone()));
    let accounts = index
        .accounts
        .into_iter()
        .map(|item| GrokAccount {
            active: current_id.as_deref() == Some(item.id.as_str()),
            id: item.id,
            name: item.name,
            email: item.email,
            user_id: item.user_id,
            updated_at: item.updated_at,
        })
        .collect();
    Ok(GrokAccountList {
        logged_in: current.is_some(),
        current_email: current.as_ref().map(|item| item.email.clone()).unwrap_or_default(),
        current_name: current.as_ref().map(|item| item.display.clone()).unwrap_or_default(),
        accounts,
    })
}

pub fn save_current(app: &AppHandle, name: String) -> Result<GrokAccountList, String> {
    let identity = read_live_identity().ok_or_else(|| "当前没有登录的 Grok 账号，请先登录。".to_string())?;
    let live = live_auth_path();
    if !live.is_file() {
        return Err("没有找到 ~/.grok/auth.json，请先 grok login。".into());
    }
    let mut index = load_index(app)?;
    let now = Utc::now().to_rfc3339();
    let label = normalize_name(&name, &identity);
    if let Some(existing) = find_meta_mut(&mut index, &identity) {
        existing.name = label;
        existing.email = identity.email.clone();
        existing.updated_at = now;
        write_account_file(app, &existing.id, &live)?;
    } else {
        let id = Uuid::new_v4().to_string();
        write_account_file(app, &id, &live)?;
        index.accounts.push(AccountMeta {
            id,
            name: label,
            email: identity.email,
            user_id: identity.user_id,
            updated_at: now,
        });
    }
    save_index(app, &index)?;
    list_accounts(app)
}

pub fn switch_account(app: &AppHandle, account_id: &str) -> Result<GrokAccountList, String> {
    if read_live_identity().is_some() {
        save_current(app, String::new())?;
    }
    let index = load_index(app)?;
    if !index.accounts.iter().any(|item| item.id == account_id) {
        return Err("没有这个 Grok 账号快照。".into());
    }
    let src = account_file(app, account_id)?;
    if !src.is_file() {
        return Err("这个账号的登录文件丢了，请重新登录并保存。".into());
    }
    let dest = live_auth_path();
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建 ~/.grok：{err}"))?;
    }
    atomic_copy(&src, &dest)?;
    list_accounts(app)
}

pub fn delete_account(app: &AppHandle, account_id: &str) -> Result<GrokAccountList, String> {
    let mut index = load_index(app)?;
    index.accounts.retain(|item| item.id != account_id);
    save_index(app, &index)?;
    let dir = accounts_dir(app)?.join(account_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| format!("删除账号快照失败：{err}"))?;
    }
    list_accounts(app)
}

pub fn login_new(app: &AppHandle, settings: &AppSettings) -> Result<GrokAccountList, String> {
    if read_live_identity().is_some() {
        save_current(app, String::new())?;
    }
    let exe = tools::resolve_binary(ToolId::Grokbuild, settings)?;
    let proxy = proxy_env(true, &settings.default_proxy_url).unwrap_or_default();
    run_grok(&exe, &["logout"], &proxy, false).ok();
    run_grok(&exe, &["login"], &proxy, true)?;
    if read_live_identity().is_none() {
        return Err("登录没有完成。请在弹出的窗口或浏览器里登完新号后再试。".into());
    }
    save_current(app, String::new())
}

fn persist_live(app: &AppHandle) -> Result<(), String> {
    let Some(identity) = read_live_identity() else {
        return Ok(());
    };
    let live = live_auth_path();
    if !live.is_file() {
        return Ok(());
    }
    let mut index = load_index(app)?;
    if let Some(existing) = find_meta_mut(&mut index, &identity) {
        existing.email = identity.email;
        existing.updated_at = Utc::now().to_rfc3339();
        write_account_file(app, &existing.id, &live)?;
        save_index(app, &index)?;
    }
    Ok(())
}

fn grok_home() -> PathBuf {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".grok"))
}

fn live_auth_path() -> PathBuf {
    grok_home().join("auth.json")
}

fn accounts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("无法定位应用数据目录：{err}"))?
        .join("grok-accounts");
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建账号目录：{err}"))?;
    Ok(dir)
}

fn index_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(accounts_dir(app)?.join("index.json"))
}

fn account_file(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err("账号编号不合法。".into());
    }
    Ok(accounts_dir(app)?.join(id).join("auth.json"))
}

fn load_index(app: &AppHandle) -> Result<AccountIndex, String> {
    let path = index_path(app)?;
    if !path.exists() {
        return Ok(AccountIndex::default());
    }
    let text = fs::read_to_string(&path).map_err(|err| format!("读取账号列表失败：{err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("账号列表损坏：{err}"))
}

fn save_index(app: &AppHandle, index: &AccountIndex) -> Result<(), String> {
    let path = index_path(app)?;
    write_atomic(&path, &serde_json::to_string_pretty(index).map_err(|err| format!("序列化账号列表失败：{err}"))?)
}

fn write_account_file(app: &AppHandle, id: &str, src: &Path) -> Result<(), String> {
    let dest = account_file(app, id)?;
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建账号目录：{err}"))?;
    }
    atomic_copy(src, &dest)
}

fn atomic_copy(src: &Path, dest: &Path) -> Result<(), String> {
    let bytes = fs::read(src).map_err(|err| format!("读取登录文件失败：{err}"))?;
    write_atomic(dest, &bytes)
}

fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|err| format!("写入失败：{err}"))?;
    fs::rename(&tmp, path).map_err(|err| format!("保存失败：{err}"))?;
    Ok(())
}

fn find_meta<'a>(index: &'a AccountIndex, identity: &Identity) -> Option<&'a AccountMeta> {
    index.accounts.iter().find(|item| {
        (!identity.user_id.is_empty() && item.user_id == identity.user_id)
            || (!identity.email.is_empty() && item.email == identity.email)
    })
}

fn find_meta_mut<'a>(index: &'a mut AccountIndex, identity: &Identity) -> Option<&'a mut AccountMeta> {
    index.accounts.iter_mut().find(|item| {
        (!identity.user_id.is_empty() && item.user_id == identity.user_id)
            || (!identity.email.is_empty() && item.email == identity.email)
    })
}

fn read_live_identity() -> Option<Identity> {
    let path = live_auth_path();
    let text = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    identity_from_auth(&value)
}

fn normalize_name(name: &str, identity: &Identity) -> String {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        return trimmed.chars().take(40).collect();
    }
    if !identity.display.is_empty() {
        return identity.display.clone();
    }
    if !identity.email.is_empty() {
        return identity.email.clone();
    }
    "Grok 账号".into()
}

fn identity_from_auth(value: &Value) -> Option<Identity> {
    let mut email = String::new();
    let mut user_id = String::new();
    let mut first = String::new();
    let mut last = String::new();
    walk_identity(value, &mut email, &mut user_id, &mut first, &mut last);
    if email.is_empty() && user_id.is_empty() {
        return None;
    }
    let display = format!("{first}{last}").trim().to_string();
    Some(Identity {
        display: if display.is_empty() { email.clone() } else { display },
        email,
        user_id,
    })
}

fn walk_identity(value: &Value, email: &mut String, user_id: &mut String, first: &mut String, last: &mut String) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match key.as_str() {
                    "email" if email.is_empty() => {
                        if let Some(text) = child.as_str().filter(|item| item.contains('@')) {
                            *email = text.to_string();
                        }
                    }
                    "user_id" if user_id.is_empty() => {
                        if let Some(text) = child.as_str().filter(|item| item.len() < 80) {
                            *user_id = text.to_string();
                        }
                    }
                    "first_name" if first.is_empty() => {
                        if let Some(text) = child.as_str() {
                            *first = text.to_string();
                        }
                    }
                    "last_name" if last.is_empty() => {
                        if let Some(text) = child.as_str() {
                            *last = text.to_string();
                        }
                    }
                    "refresh_token" | "access_token" | "key" | "id_token" => {}
                    _ => walk_identity(child, email, user_id, first, last),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_identity(item, email, user_id, first, last);
            }
        }
        _ => {}
    }
}

fn run_grok(exe: &Path, args: &[&str], proxy: &[(String, String)], visible: bool) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    platform::prepare_command(&mut cmd);
    for (key, value) in proxy {
        cmd.env(key, value);
    }
    apply_window(&mut cmd, visible);
    let output = cmd
        .output()
        .map_err(|err| format!("无法启动 {}：{err}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut text = String::new();
    text.push_str(&stdout);
    if !stderr.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    if output.status.success() {
        Ok(text)
    } else if !text.trim().is_empty() {
        Err(text)
    } else {
        Err(format!("{} 退出码 {}", exe.display(), output.status.code().unwrap_or(-1)))
    }
}

fn apply_window(cmd: &mut Command, visible: bool) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        cmd.creation_flags(if visible { CREATE_NEW_CONSOLE } else { CREATE_NO_WINDOW });
    }
    #[cfg(not(windows))]
    {
        let _ = (cmd, visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_email_from_issuer_slot() {
        let value = json!({
            "https://auth.x.ai::abc": {
                "email": "dev@example.com",
                "user_id": "user-1",
                "first_name": "A",
                "last_name": "B",
                "refresh_token": "secret"
            }
        });
        let id = identity_from_auth(&value).unwrap();
        assert_eq!(id.email, "dev@example.com");
        assert_eq!(id.user_id, "user-1");
        assert_eq!(id.display, "AB");
    }
}
