use crate::platform;
use crate::proxy::proxy_env;
use crate::state::AppSettings;
use crate::tools::{self, ToolId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const ANTHROPIC_ENV_KEYS: [&str; 3] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"];
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub account_uuid: String,
    pub subscription_type: String,
    pub updated_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountList {
    pub logged_in: bool,
    pub current_email: String,
    pub current_name: String,
    pub accounts: Vec<ClaudeAccount>,
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
    account_uuid: String,
    #[serde(default)]
    subscription_type: String,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct Identity {
    email: String,
    account_uuid: String,
    display: String,
    subscription_type: String,
}

#[derive(Debug, Clone)]
struct LivePaths {
    credentials: PathBuf,
    claude_json: PathBuf,
    settings: PathBuf,
}

pub fn list_accounts(app: &AppHandle) -> Result<ClaudeAccountList, String> {
    list_accounts_in(&accounts_dir(app)?, &live_paths())
}

pub fn save_current(app: &AppHandle, name: String) -> Result<ClaudeAccountList, String> {
    save_current_in(&accounts_dir(app)?, &live_paths(), name)
}

pub fn switch_account(app: &AppHandle, account_id: &str) -> Result<ClaudeAccountList, String> {
    switch_account_in(&accounts_dir(app)?, &live_paths(), account_id)
}

pub fn delete_account(app: &AppHandle, account_id: &str) -> Result<ClaudeAccountList, String> {
    delete_account_in(&accounts_dir(app)?, account_id, &live_paths())
}

pub fn login_new(app: &AppHandle, settings: &AppSettings) -> Result<ClaudeAccountList, String> {
    let store = accounts_dir(app)?;
    let live = live_paths();
    if read_live_identity(&live).is_some() {
        save_current_in(&store, &live, String::new())?;
    }
    clear_live_auth(&live)?;
    let exe = tools::resolve_binary(ToolId::Claude, settings)?;
    let proxy = proxy_env(true, &settings.default_proxy_url).unwrap_or_default();
    run_claude(&exe, &["auth", "login", "--claudeai"], &proxy, true)?;
    if read_live_identity(&live).is_none() {
        return Err("登录没有完成。请在弹出的窗口或浏览器里登完新号后再试。".into());
    }
    save_current_in(&store, &live, String::new())
}

fn list_accounts_in(store: &Path, live: &LivePaths) -> Result<ClaudeAccountList, String> {
    persist_live(store, live)?;
    let current = read_live_identity(live);
    let index = load_index(store)?;
    let current_id = current
        .as_ref()
        .and_then(|id| find_meta(&index, id).map(|item| item.id.clone()));
    let accounts = index
        .accounts
        .into_iter()
        .map(|item| ClaudeAccount {
            active: current_id.as_deref() == Some(item.id.as_str()),
            id: item.id,
            name: item.name,
            email: item.email,
            account_uuid: item.account_uuid,
            subscription_type: item.subscription_type,
            updated_at: item.updated_at,
        })
        .collect();
    Ok(ClaudeAccountList {
        logged_in: current.is_some(),
        current_email: current.as_ref().map(|item| item.email.clone()).unwrap_or_default(),
        current_name: current.as_ref().map(|item| item.display.clone()).unwrap_or_default(),
        accounts,
    })
}

fn save_current_in(store: &Path, live: &LivePaths, name: String) -> Result<ClaudeAccountList, String> {
    let identity = read_live_identity(live).ok_or_else(|| "当前没有登录的 Claude 账号，请先登录。".to_string())?;
    let oauth = read_claude_ai_oauth(live).ok_or_else(|| "没有找到 Claude 登录文件，请先 claude auth login。".to_string())?;
    let account = read_oauth_account(live)?.ok_or_else(|| "没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。".to_string())?;
    let mut index = load_index(store)?;
    let now = Utc::now().to_rfc3339();
    let label = normalize_name(&name, &identity);
    if let Some(existing) = find_meta_mut(&mut index, &identity) {
        existing.name = label;
        existing.email = identity.email.clone();
        existing.account_uuid = identity.account_uuid.clone();
        existing.subscription_type = identity.subscription_type.clone();
        existing.updated_at = now;
        write_snapshot(store, &existing.id, &oauth, &account)?;
    } else {
        let id = Uuid::new_v4().to_string();
        write_snapshot(store, &id, &oauth, &account)?;
        index.accounts.push(AccountMeta {
            id,
            name: label,
            email: identity.email,
            account_uuid: identity.account_uuid,
            subscription_type: identity.subscription_type,
            updated_at: now,
        });
    }
    save_index(store, &index)?;
    list_accounts_in(store, live)
}

fn switch_account_in(store: &Path, live: &LivePaths, account_id: &str) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    if read_live_identity(live).is_some() {
        save_current_in(store, live, String::new())?;
    }
    let index = load_index(store)?;
    if !index.accounts.iter().any(|item| item.id == account_id) {
        return Err("没有这个 Claude 账号快照。".into());
    }
    let cred_path = snapshot_credentials(store, account_id)?;
    let account_path = snapshot_oauth_account(store, account_id)?;
    if !cred_path.is_file() || !account_path.is_file() {
        return Err("这个账号的登录文件丢了，请重新登录并保存。".into());
    }
    let cred: Value = read_json(&cred_path)?;
    let oauth = cred
        .get("claudeAiOauth")
        .cloned()
        .filter(|item| item.is_object())
        .ok_or_else(|| "这个账号的登录文件丢了，请重新登录并保存。".to_string())?;
    let account = read_json(&account_path)?;
    if !account.is_object() {
        return Err("这个账号的登录文件丢了，请重新登录并保存。".into());
    }
    write_live_oauth(live, &oauth)?;
    write_live_oauth_account(live, Some(&account))?;
    strip_anthropic_env(&live.settings)?;
    list_accounts_in(store, live)
}

fn delete_account_in(store: &Path, account_id: &str, live: &LivePaths) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    let mut index = load_index(store)?;
    index.accounts.retain(|item| item.id != account_id);
    save_index(store, &index)?;
    let dir = store.join(account_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| format!("删除账号快照失败：{err}"))?;
    }
    list_accounts_in(store, live)
}

fn persist_live(store: &Path, live: &LivePaths) -> Result<(), String> {
    let Some(identity) = read_live_identity(live) else {
        return Ok(());
    };
    let Some(oauth) = read_claude_ai_oauth(live) else {
        return Ok(());
    };
    let Ok(Some(account)) = read_oauth_account(live) else {
        return Ok(());
    };
    let mut index = load_index(store)?;
    if let Some(existing) = find_meta_mut(&mut index, &identity) {
        existing.email = identity.email;
        existing.account_uuid = identity.account_uuid;
        existing.subscription_type = identity.subscription_type;
        existing.updated_at = Utc::now().to_rfc3339();
        write_snapshot(store, &existing.id, &oauth, &account)?;
        save_index(store, &index)?;
    }
    Ok(())
}

fn live_paths() -> LivePaths {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let claude_home = tools::claude_home();
    LivePaths {
        credentials: claude_home.join(".credentials.json"),
        claude_json: home.join(".claude.json"),
        settings: claude_home.join("settings.json"),
    }
}

fn accounts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("无法定位应用数据目录：{err}"))?
        .join("claude-accounts");
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建账号目录：{err}"))?;
    Ok(dir)
}

fn index_path(store: &Path) -> PathBuf {
    store.join("index.json")
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err("账号编号不合法。".into());
    }
    Ok(())
}

fn snapshot_credentials(store: &Path, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    Ok(store.join(id).join("credentials.json"))
}

fn snapshot_oauth_account(store: &Path, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    Ok(store.join(id).join("oauth-account.json"))
}

fn load_index(store: &Path) -> Result<AccountIndex, String> {
    let path = index_path(store);
    if !path.exists() {
        return Ok(AccountIndex::default());
    }
    let text = fs::read_to_string(&path).map_err(|err| format!("读取账号列表失败：{err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("账号列表损坏：{err}"))
}

fn save_index(store: &Path, index: &AccountIndex) -> Result<(), String> {
    fs::create_dir_all(store).map_err(|err| format!("无法创建账号目录：{err}"))?;
    write_atomic(
        &index_path(store),
        serde_json::to_string_pretty(index).map_err(|err| format!("序列化账号列表失败：{err}"))?,
        false,
    )
}

fn write_snapshot(store: &Path, id: &str, oauth: &Value, account: &Value) -> Result<(), String> {
    let cred_path = snapshot_credentials(store, id)?;
    if let Some(dir) = cred_path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建账号目录：{err}"))?;
    }
    let wrapped = json!({ "claudeAiOauth": oauth });
    write_atomic(
        &cred_path,
        serde_json::to_vec_pretty(&wrapped).map_err(|err| format!("序列化登录文件失败：{err}"))?,
        true,
    )?;
    write_atomic(
        &snapshot_oauth_account(store, id)?,
        serde_json::to_vec_pretty(account).map_err(|err| format!("序列化账号缓存失败：{err}"))?,
        false,
    )
}

fn write_live_oauth(live: &LivePaths, oauth: &Value) -> Result<(), String> {
    let mut root = read_json_object_or_empty(&live.credentials)?;
    root.insert("claudeAiOauth".into(), oauth.clone());
    if let Some(dir) = live.credentials.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建 Claude 配置目录：{err}"))?;
    }
    write_atomic(
        &live.credentials,
        serde_json::to_vec_pretty(&Value::Object(root)).map_err(|err| format!("序列化登录文件失败：{err}"))?,
        true,
    )?;
    sync_keychain_oauth(Some(oauth))
}

fn write_live_oauth_account(live: &LivePaths, account: Option<&Value>) -> Result<(), String> {
    if !live.claude_json.is_file() {
        return Err("没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。".into());
    }
    let mut root = read_json_object(&live.claude_json, "没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。")?;
    match account {
        Some(value) => {
            root.insert("oauthAccount".into(), value.clone());
        }
        None => {
            root.remove("oauthAccount");
        }
    }
    write_atomic(
        &live.claude_json,
        serde_json::to_vec_pretty(&Value::Object(root)).map_err(|err| format!("序列化账号缓存失败：{err}"))?,
        false,
    )
}

fn clear_live_auth(live: &LivePaths) -> Result<(), String> {
    if live.credentials.is_file() {
        let mut root = read_json_object_or_empty(&live.credentials)?;
        root.remove("claudeAiOauth");
        write_atomic(
            &live.credentials,
            serde_json::to_vec_pretty(&Value::Object(root)).map_err(|err| format!("序列化登录文件失败：{err}"))?,
            true,
        )?;
    }
    if live.claude_json.is_file() {
        write_live_oauth_account(live, None)?;
    }
    sync_keychain_oauth(None)
}

fn strip_anthropic_env(settings_path: &Path) -> Result<(), String> {
    if !settings_path.is_file() {
        return Ok(());
    }
    let mut root = read_json_object(settings_path, "读取 Claude settings.json 失败")?;
    if let Some(Value::Object(env)) = root.get_mut("env") {
        for key in ANTHROPIC_ENV_KEYS {
            env.remove(key);
        }
    }
    write_atomic(
        settings_path,
        serde_json::to_vec_pretty(&Value::Object(root)).map_err(|err| format!("序列化 settings.json 失败：{err}"))?,
        false,
    )
}

fn read_live_identity(live: &LivePaths) -> Option<Identity> {
    let account = read_oauth_account(live).ok().flatten()?;
    identity_from_account(&account, read_claude_ai_oauth(live).as_ref())
}

fn read_oauth_account(live: &LivePaths) -> Result<Option<Value>, String> {
    if !live.claude_json.is_file() {
        return Ok(None);
    }
    let root = read_json(&live.claude_json)?;
    Ok(root.get("oauthAccount").cloned().filter(|item| item.is_object()))
}

fn read_claude_ai_oauth(live: &LivePaths) -> Option<Value> {
    if let Ok(root) = read_json(&live.credentials) {
        if let Some(oauth) = root.get("claudeAiOauth").cloned().filter(|item| item.is_object()) {
            return Some(oauth);
        }
    }
    keychain_read_oauth()
}

fn identity_from_account(account: &Value, oauth: Option<&Value>) -> Option<Identity> {
    let email = account
        .get("emailAddress")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let account_uuid = account
        .get("accountUuid")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if email.is_empty() && account_uuid.is_empty() {
        return None;
    }
    let display_raw = account
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .or_else(|| {
            account
                .get("fullName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
        })
        .unwrap_or("")
        .to_string();
    let subscription_type = oauth
        .and_then(|item| item.get("subscriptionType"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    Some(Identity {
        display: if display_raw.is_empty() { email.clone() } else { display_raw },
        email,
        account_uuid,
        subscription_type,
    })
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
    "Claude 账号".into()
}

fn find_meta<'a>(index: &'a AccountIndex, identity: &Identity) -> Option<&'a AccountMeta> {
    index.accounts.iter().find(|item| identity_matches(item, identity))
}

fn find_meta_mut<'a>(index: &'a mut AccountIndex, identity: &Identity) -> Option<&'a mut AccountMeta> {
    index.accounts.iter_mut().find(|item| identity_matches(item, identity))
}

fn identity_matches(item: &AccountMeta, identity: &Identity) -> bool {
    (!identity.account_uuid.is_empty() && item.account_uuid == identity.account_uuid)
        || (!identity.email.is_empty() && item.email == identity.email)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{} 损坏：{err}", path.display()))
}

fn read_json_object(path: &Path, missing: &str) -> Result<Map<String, Value>, String> {
    if !path.is_file() {
        return Err(missing.into());
    }
    match read_json(path)? {
        Value::Object(map) => Ok(map),
        _ => Err(format!("{} 损坏：根节点不是对象", path.display())),
    }
}

fn read_json_object_or_empty(path: &Path) -> Result<Map<String, Value>, String> {
    if !path.is_file() {
        return Ok(Map::new());
    }
    match read_json(path)? {
        Value::Object(map) => Ok(map),
        _ => Ok(Map::new()),
    }
}

fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>, secret: bool) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建目录：{err}"))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|err| format!("写入失败：{err}"))?;
    fs::rename(&tmp, path).map_err(|err| format!("保存失败：{err}"))?;
    if secret {
        tighten_secret(path);
    }
    Ok(())
}

fn tighten_secret(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn sync_keychain_oauth(oauth: Option<&Value>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        keychain_delete();
        if let Some(value) = oauth {
            let payload = serde_json::to_string(&json!({ "claudeAiOauth": value }))
                .map_err(|err| format!("序列化钥匙串失败：{err}"))?;
            let output = Command::new("security")
                .args([
                    "add-generic-password",
                    "-s",
                    KEYCHAIN_SERVICE,
                    "-a",
                    "Claude Code",
                    "-w",
                    &payload,
                ])
                .output()
                .map_err(|err| format!("无法写入 macOS 钥匙串中的 Claude 登录：{err}"))?;
            if !output.status.success() {
                return Err("无法写入 macOS 钥匙串中的 Claude 登录。".into());
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = oauth;
        Ok(())
    }
}

fn keychain_read_oauth() -> Option<Value> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let value: Value = serde_json::from_str(text.trim()).ok()?;
        value
            .get("claudeAiOauth")
            .cloned()
            .filter(|item| item.is_object())
            .or_else(|| value.is_object().then_some(value))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn keychain_delete() {
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", KEYCHAIN_SERVICE])
        .output();
}

fn run_claude(exe: &Path, args: &[&str], proxy: &[(String, String)], visible: bool) -> Result<String, String> {
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

    fn fixture() -> (tempfile::TempDir, PathBuf, LivePaths) {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("store");
        let claude_home = root.path().join("claude");
        let home = root.path().join("home");
        fs::create_dir_all(&claude_home).unwrap();
        fs::create_dir_all(&home).unwrap();
        let live = LivePaths {
            credentials: claude_home.join(".credentials.json"),
            claude_json: home.join(".claude.json"),
            settings: claude_home.join("settings.json"),
        };
        (root, store, live)
    }

    fn seed_live(live: &LivePaths, email: &str, uuid: &str, token: &str) {
        write_atomic(
            &live.credentials,
            serde_json::to_vec_pretty(&json!({
                "claudeAiOauth": {
                    "accessToken": token,
                    "refreshToken": format!("{token}-refresh"),
                    "subscriptionType": "pro"
                },
                "mcpOAuth": { "keep": true }
            }))
            .unwrap(),
            true,
        )
        .unwrap();
        write_atomic(
            &live.claude_json,
            serde_json::to_vec_pretty(&json!({
                "projects": { "demo": {} },
                "mcpServers": { "keep": true },
                "oauthAccount": {
                    "accountUuid": uuid,
                    "emailAddress": email,
                    "displayName": "Ada",
                    "fullName": "Ada Lovelace"
                }
            }))
            .unwrap(),
            false,
        )
        .unwrap();
    }

    #[test]
    fn reads_identity_from_oauth_account_and_subscription() {
        let account = json!({
            "accountUuid": "acct-1",
            "emailAddress": "ada@example.com",
            "displayName": "Ada",
            "fullName": "Ada Lovelace"
        });
        let oauth = json!({ "subscriptionType": "pro", "accessToken": "secret" });
        let id = identity_from_account(&account, Some(&oauth)).unwrap();
        assert_eq!(id.email, "ada@example.com");
        assert_eq!(id.account_uuid, "acct-1");
        assert_eq!(id.display, "Ada");
        assert_eq!(id.subscription_type, "pro");
    }

    #[test]
    fn write_oauth_preserves_mcp_oauth() {
        let (_root, _store, live) = fixture();
        write_atomic(
            &live.credentials,
            serde_json::to_vec_pretty(&json!({
                "claudeAiOauth": { "accessToken": "old" },
                "mcpOAuth": { "keep": true }
            }))
            .unwrap(),
            true,
        )
        .unwrap();
        write_live_oauth(&live, &json!({ "accessToken": "new", "subscriptionType": "pro" })).unwrap();
        let root = read_json(&live.credentials).unwrap();
        assert_eq!(root["claudeAiOauth"]["accessToken"], "new");
        assert_eq!(root["mcpOAuth"]["keep"], true);
    }

    #[test]
    fn write_oauth_account_preserves_projects() {
        let (_root, _store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        write_live_oauth_account(
            &live,
            Some(&json!({
                "accountUuid": "acct-2",
                "emailAddress": "bob@example.com"
            })),
        )
        .unwrap();
        let root = read_json(&live.claude_json).unwrap();
        assert_eq!(root["oauthAccount"]["emailAddress"], "bob@example.com");
        assert_eq!(root["projects"]["demo"], json!({}));
        assert_eq!(root["mcpServers"]["keep"], true);
    }

    #[test]
    fn clear_live_keeps_other_keys() {
        let (_root, _store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        clear_live_auth(&live).unwrap();
        let cred = read_json(&live.credentials).unwrap();
        assert!(cred.get("claudeAiOauth").is_none());
        assert_eq!(cred["mcpOAuth"]["keep"], true);
        let cfg = read_json(&live.claude_json).unwrap();
        assert!(cfg.get("oauthAccount").is_none());
        assert_eq!(cfg["projects"]["demo"], json!({}));
    }

    #[test]
    fn switch_strips_anthropic_env_keys() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let list = save_current_in(&store, &live, "工作号".into()).unwrap();
        let ada_id = list.accounts[0].id.clone();

        write_atomic(
            &live.settings,
            serde_json::to_vec_pretty(&json!({
                "env": {
                    "ANTHROPIC_API_KEY": "sk-old",
                    "ANTHROPIC_AUTH_TOKEN": "tok",
                    "ANTHROPIC_BASE_URL": "https://example.invalid",
                    "KEEP_ME": "yes"
                }
            }))
            .unwrap(),
            false,
        )
        .unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        save_current_in(&store, &live, "个人号".into()).unwrap();

        switch_account_in(&store, &live, &ada_id).unwrap();
        let settings = read_json(&live.settings).unwrap();
        let env = settings["env"].as_object().unwrap();
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"));
        assert!(!env.contains_key("ANTHROPIC_BASE_URL"));
        assert_eq!(env["KEEP_ME"], "yes");
        let cred = read_json(&live.credentials).unwrap();
        assert_eq!(cred["claudeAiOauth"]["accessToken"], "tok-a");
        let cfg = read_json(&live.claude_json).unwrap();
        assert_eq!(cfg["oauthAccount"]["emailAddress"], "ada@example.com");
    }

    #[test]
    fn save_matches_account_uuid_without_duplicate() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        save_current_in(&store, &live, "工作号".into()).unwrap();
        write_atomic(
            &live.credentials,
            serde_json::to_vec_pretty(&json!({
                "claudeAiOauth": {
                    "accessToken": "tok-a2",
                    "subscriptionType": "pro"
                }
            }))
            .unwrap(),
            true,
        )
        .unwrap();
        let list = save_current_in(&store, &live, "公司号".into()).unwrap();
        assert_eq!(list.accounts.len(), 1);
        assert_eq!(list.accounts[0].name, "公司号");
        let snap = read_json(&snapshot_credentials(&store, &list.accounts[0].id).unwrap()).unwrap();
        assert_eq!(snap["claudeAiOauth"]["accessToken"], "tok-a2");
    }

    #[test]
    fn delete_snapshot_leaves_live() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let list = save_current_in(&store, &live, "工作号".into()).unwrap();
        let id = list.accounts[0].id.clone();
        let after = delete_account_in(&store, &id, &live).unwrap();
        assert!(after.accounts.is_empty());
        assert!(after.logged_in);
        assert_eq!(after.current_email, "ada@example.com");
        assert!(live.credentials.is_file());
        assert!(!store.join(&id).exists());
    }

    #[test]
    fn rejects_illegal_account_id() {
        let (_root, store, live) = fixture();
        let err = delete_account_in(&store, "../x", &live).unwrap_err();
        assert_eq!(err, "账号编号不合法。");
        let err = switch_account_in(&store, &live, "abc.json").unwrap_err();
        assert_eq!(err, "账号编号不合法。");
    }
}
