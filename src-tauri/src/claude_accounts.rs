use crate::claude_oauth::{self, AuthError, HttpOAuthClient, OAuthClient};
use crate::platform;
use crate::proxy::proxy_env;
use crate::state::AppSettings;
use crate::tools::{self, ToolId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const ANTHROPIC_ENV_KEYS: [&str; 3] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
];
static AUTH_OPERATION: Mutex<()> = Mutex::new(());
#[cfg(all(target_os = "macos", not(test)))]
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
    let _guard = lock_operation()?;
    list_accounts_in(&accounts_dir(app)?, &live_paths())
}

pub fn lock_operation() -> Result<MutexGuard<'static, ()>, String> {
    AUTH_OPERATION
        .try_lock()
        .map_err(|_| "Claude 账号正在处理，请稍后再试。".into())
}

pub fn save_current(
    app: &AppHandle,
    settings: &AppSettings,
    name: String,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let result = save_current_in(
        &store,
        &live_paths(),
        name,
        &HttpOAuthClient::new(settings)?,
    );
    diagnostic(&store, "save_finished", json!({ "ok": result.is_ok() }));
    result
}

pub fn switch_account(
    app: &AppHandle,
    settings: &AppSettings,
    account_id: &str,
    stop_terminals: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    diagnostic(&store, "switch_started", json!({ "accountId": account_id }));
    let result = switch_account_in(
        &store,
        &live_paths(),
        account_id,
        &HttpOAuthClient::new(settings)?,
        stop_terminals,
    );
    diagnostic(
        &store,
        "switch_finished",
        json!({ "accountId": account_id, "ok": result.is_ok() }),
    );
    result
}

pub fn delete_account(app: &AppHandle, account_id: &str) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    delete_account_in(&accounts_dir(app)?, account_id, &live_paths())
}

pub fn login_new(
    app: &AppHandle,
    settings: &AppSettings,
    stop_terminals: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let live = live_paths();
    let exe = tools::resolve_binary(ToolId::Claude, settings)?;
    let proxy = proxy_env(
        !settings.default_proxy_url.trim().is_empty(),
        &settings.default_proxy_url,
    )?;
    let client = HttpOAuthClient::new(settings)?;
    stop_terminals()?;
    preserve_current(&store, &live, &client)?;
    let backup = LiveBackup::capture(&live)?;
    diagnostic(&store, "login_started", json!({}));
    let result = (|| {
        clear_live_auth(&live)?;
        strip_anthropic_env(&live.settings)?;
        run_claude(&exe, &["auth", "login", "--claudeai"], &proxy, true)?;
        save_current_in(&store, &live, String::new(), &client)
    })();
    if result.is_err() {
        rollback(&store, &backup)?;
    }
    diagnostic(&store, "login_finished", json!({ "ok": result.is_ok() }));
    result
}

fn list_accounts_in(store: &Path, live: &LivePaths) -> Result<ClaudeAccountList, String> {
    // Listing must never overwrite a snapshot from an unverified live identity.
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
        current_email: current
            .as_ref()
            .map(|item| item.email.clone())
            .unwrap_or_default(),
        current_name: current
            .as_ref()
            .map(|item| item.display.clone())
            .unwrap_or_default(),
        accounts,
    })
}

fn save_current_in(
    store: &Path,
    live: &LivePaths,
    name: String,
    client: &impl OAuthClient,
) -> Result<ClaudeAccountList, String> {
    let oauth = read_claude_ai_oauth(live)
        .filter(claude_oauth::has_credentials)
        .ok_or_else(|| AuthError::Empty.to_string())?;
    let account = read_oauth_account(live)?
        .ok_or_else(|| "没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。".to_string())?;
    let oauth = verified_oauth(store, oauth, &account, client, |next| {
        write_live_oauth(live, next)
    })?;
    let identity = identity_from_account(&account, Some(&oauth))
        .ok_or_else(|| "Claude 账号信息不完整，请重新登录。".to_string())?;
    let mut index = load_index(store)?;
    let now = Utc::now().to_rfc3339();
    let label = normalize_name(&name, &identity);
    if let Some(existing) = find_meta_mut(&mut index, &identity) {
        if !name.trim().is_empty() {
            existing.name = label;
        }
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

fn switch_account_in(
    store: &Path,
    live: &LivePaths,
    account_id: &str,
    client: &impl OAuthClient,
    stop_terminals: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    let index = load_index(store)?;
    let meta = index
        .accounts
        .iter()
        .find(|item| item.id == account_id)
        .ok_or_else(|| "没有这个 Claude 账号快照。".to_string())?;
    let cred_path = snapshot_credentials(store, account_id)?;
    let account_path = snapshot_oauth_account(store, account_id)?;
    if !cred_path.is_file() || !account_path.is_file() {
        return Err("这个账号的登录文件丢了，请重新登录并保存。".into());
    }
    let pending = cred_path.with_file_name("pending-credentials.json");
    let cred = read_json(if pending.is_file() {
        &pending
    } else {
        &cred_path
    })?;
    let oauth = cred
        .get("claudeAiOauth")
        .cloned()
        .filter(claude_oauth::has_credentials)
        .ok_or_else(|| AuthError::Empty.to_string())?;
    let account = read_json(&account_path)?;
    let identity = identity_from_account(&account, Some(&oauth))
        .ok_or_else(|| "这个账号的信息不完整，请重新登录并保存。".to_string())?;
    if !identity_matches(meta, &identity) {
        return Err(AuthError::IdentityMismatch.to_string());
    }
    // No Claude process owned by Dock may still be writing the outgoing login.
    stop_terminals()?;
    diagnostic(store, "terminals_stopped", json!({}));
    preserve_current(store, live, client)?;
    // The outgoing account may have been the selected account; re-read its latest
    // credentials after preservation instead of restoring an older token copy.
    let cred = read_json(if pending.is_file() {
        &pending
    } else {
        &cred_path
    })?;
    let oauth = cred
        .get("claudeAiOauth")
        .cloned()
        .ok_or_else(|| AuthError::Empty.to_string())?;
    let oauth = verified_oauth(store, oauth, &account, client, |next| {
        // Refresh tokens rotate. Keep the new grant even if the later profile
        // request fails, and resume from it on the next attempt.
        write_atomic(
            &pending,
            serde_json::to_vec_pretty(&json!({ "claudeAiOauth": next }))
                .map_err(|_| "保存续期凭据失败。")?,
            true,
        )
    })?;
    let backup = LiveBackup::capture(live)?;
    let result = (|| {
        write_live_oauth(live, &oauth)?;
        write_live_oauth_account(live, Some(&account))?;
        strip_anthropic_env(&live.settings)?;
        write_snapshot(store, account_id, &oauth, &account)?;
        list_accounts_in(store, live)
    })();
    if result.is_err() {
        rollback(store, &backup)?;
    } else if pending.exists() {
        let _ = fs::remove_file(&pending);
    }
    result
}

fn delete_account_in(
    store: &Path,
    account_id: &str,
    live: &LivePaths,
) -> Result<ClaudeAccountList, String> {
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

fn preserve_current(
    store: &Path,
    live: &LivePaths,
    client: &impl OAuthClient,
) -> Result<(), String> {
    if read_live_identity(live).is_some() {
        let result = save_current_in(store, live, String::new(), client);
        if result.is_err() {
            // An invalid outgoing identity must not block switching to a valid
            // account. Preserve its raw grant separately, never under a possibly
            // incorrect account label, before allowing the live file to change.
            let oauth =
                read_claude_ai_oauth(live).ok_or("无法保留当前 Claude 凭据，已取消账号操作。")?;
            let account = read_oauth_account(live)?;
            let recovery = store.join(format!("unverified-live-{}.json", Uuid::new_v4()));
            write_atomic(
                &recovery,
                serde_json::to_vec_pretty(&json!({
                    "claudeAiOauth": oauth, "oauthAccount": account,
                }))
                .map_err(|_| "无法备份当前 Claude 登录。")?,
                true,
            )?;
        }
        diagnostic(
            store,
            "preserve_current",
            json!({ "verified": result.is_ok(), "recoverySaved": result.is_err() }),
        );
    }
    Ok(())
}

fn verified_oauth(
    store: &Path,
    mut oauth: Value,
    account: &Value,
    client: &impl OAuthClient,
    mut checkpoint: impl FnMut(&Value) -> Result<(), String>,
) -> Result<Value, String> {
    if !claude_oauth::has_credentials(&oauth) {
        return Err(AuthError::Empty.to_string());
    }
    diagnostic(
        store,
        "verify_started",
        json!({
            "hasAccessToken": claude_oauth::token(&oauth, "accessToken").is_some(),
            "hasRefreshToken": claude_oauth::token(&oauth, "refreshToken").is_some(),
            "expiresAt": oauth.get("expiresAt").and_then(Value::as_i64),
        }),
    );
    let refresh_first = claude_oauth::needs_refresh(&oauth);
    if refresh_first {
        oauth = logged_auth(store, "refresh", client.refresh(&oauth))?;
        checkpoint(&oauth)?;
    }
    let profile = match client.profile(&oauth) {
        Err(AuthError::Rejected) if !refresh_first => {
            oauth = logged_auth(store, "refresh_after_rejection", client.refresh(&oauth))?;
            checkpoint(&oauth)?;
            logged_auth(store, "profile", client.profile(&oauth))?
        }
        result => logged_auth(store, "profile", result)?,
    };
    logged_auth(
        store,
        "identity",
        claude_oauth::verify_identity(account, &profile),
    )?;
    Ok(oauth)
}

fn logged_auth<T>(store: &Path, stage: &str, result: Result<T, AuthError>) -> Result<T, String> {
    let mut fields = json!({ "ok": result.is_ok() });
    if let Err(err) = &result {
        fields["code"] = json!(err.code());
        if let AuthError::Http(_, status) = err {
            fields["httpStatus"] = json!(status);
        }
    }
    diagnostic(store, stage, fields);
    result.map_err(|err| err.to_string())
}

fn diagnostic(store: &Path, event: &str, fields: Value) {
    // Callers provide only stage names, IDs, booleans and timestamps. Never log
    // OAuth values, server response bodies or raw command/transport errors.
    let _ = (|| -> std::io::Result<()> {
        fs::create_dir_all(store)?;
        let path = store.join("auth-diagnostics.jsonl");
        if fs::metadata(&path).is_ok_and(|meta| meta.len() > 1_048_576) {
            fs::rename(&path, store.join("auth-diagnostics.previous.jsonl"))?;
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(
            file,
            "{}",
            json!({ "at": Utc::now().to_rfc3339(), "event": event, "details": fields })
        )
    })();
}

struct LiveBackup {
    files: Vec<(PathBuf, Option<Vec<u8>>, bool)>,
    oauth: Option<Value>,
}

impl LiveBackup {
    fn capture(live: &LivePaths) -> Result<Self, String> {
        let mut files = Vec::new();
        for (path, secret) in [
            (&live.credentials, true),
            (&live.claude_json, false),
            (&live.settings, false),
        ] {
            let bytes = match fs::read(path) {
                Ok(bytes) => Some(bytes),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                Err(err) => return Err(format!("无法备份 Claude 配置：{err}")),
            };
            files.push((path.clone(), bytes, secret));
        }
        Ok(Self {
            files,
            oauth: read_claude_ai_oauth(live),
        })
    }

    fn restore(&self) -> Result<(), String> {
        let mut failed = false;
        for (path, bytes, secret) in &self.files {
            let result = match bytes {
                Some(bytes) => write_atomic(path, bytes, *secret),
                None if path.exists() => fs::remove_file(path).map_err(|err| err.to_string()),
                None => Ok(()),
            };
            failed |= result.is_err();
        }
        failed |= sync_keychain_oauth(self.oauth.as_ref()).is_err();
        if failed {
            Err("恢复原 Claude 登录失败，请检查文件权限并重新登录。".into())
        } else {
            Ok(())
        }
    }
}

fn rollback(store: &Path, backup: &LiveBackup) -> Result<(), String> {
    let result = backup.restore();
    diagnostic(store, "rollback", json!({ "ok": result.is_ok() }));
    result
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
    )?;
    let pending = cred_path.with_file_name("pending-credentials.json");
    if pending.exists() {
        fs::remove_file(pending).map_err(|_| "清理旧续期凭据失败，请检查账号目录权限。")?;
    }
    Ok(())
}

fn write_live_oauth(live: &LivePaths, oauth: &Value) -> Result<(), String> {
    let mut root = read_json_object_or_empty(&live.credentials)?;
    root.insert("claudeAiOauth".into(), oauth.clone());
    if let Some(dir) = live.credentials.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("无法创建 Claude 配置目录：{err}"))?;
    }
    write_atomic(
        &live.credentials,
        serde_json::to_vec_pretty(&Value::Object(root))
            .map_err(|err| format!("序列化登录文件失败：{err}"))?,
        true,
    )?;
    sync_keychain_oauth(Some(oauth))
}

fn write_live_oauth_account(live: &LivePaths, account: Option<&Value>) -> Result<(), String> {
    if !live.claude_json.is_file() {
        return Err("没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。".into());
    }
    let mut root = read_json_object(
        &live.claude_json,
        "没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。",
    )?;
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
        serde_json::to_vec_pretty(&Value::Object(root))
            .map_err(|err| format!("序列化账号缓存失败：{err}"))?,
        false,
    )
}

fn clear_live_auth(live: &LivePaths) -> Result<(), String> {
    if live.credentials.is_file() {
        let mut root = read_json_object_or_empty(&live.credentials)?;
        root.remove("claudeAiOauth");
        write_atomic(
            &live.credentials,
            serde_json::to_vec_pretty(&Value::Object(root))
                .map_err(|err| format!("序列化登录文件失败：{err}"))?,
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
        serde_json::to_vec_pretty(&Value::Object(root))
            .map_err(|err| format!("序列化 settings.json 失败：{err}"))?,
        false,
    )
}

fn read_live_identity(live: &LivePaths) -> Option<Identity> {
    let oauth = read_claude_ai_oauth(live).filter(claude_oauth::has_credentials)?;
    let account = read_oauth_account(live).ok().flatten()?;
    identity_from_account(&account, Some(&oauth))
}

fn read_oauth_account(live: &LivePaths) -> Result<Option<Value>, String> {
    if !live.claude_json.is_file() {
        return Ok(None);
    }
    let root = read_json(&live.claude_json)?;
    Ok(root
        .get("oauthAccount")
        .cloned()
        .filter(|item| item.is_object()))
}

fn read_claude_ai_oauth(live: &LivePaths) -> Option<Value> {
    if let Ok(root) = read_json(&live.credentials) {
        if let Some(oauth) = root
            .get("claudeAiOauth")
            .cloned()
            .filter(|item| item.is_object())
        {
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
        display: if display_raw.is_empty() {
            email.clone()
        } else {
            display_raw
        },
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
    index
        .accounts
        .iter()
        .find(|item| identity_matches(item, identity))
}

fn find_meta_mut<'a>(
    index: &'a mut AccountIndex,
    identity: &Identity,
) -> Option<&'a mut AccountMeta> {
    index
        .accounts
        .iter_mut()
        .find(|item| identity_matches(item, identity))
}

fn identity_matches(item: &AccountMeta, identity: &Identity) -> bool {
    if !identity.account_uuid.is_empty() && !item.account_uuid.is_empty() {
        item.account_uuid == identity.account_uuid
    } else {
        !identity.email.is_empty() && item.email.eq_ignore_ascii_case(&identity.email)
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
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
    let tmp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    fs::write(&tmp, bytes).map_err(|err| format!("写入失败：{err}"))?;
    if secret {
        tighten_secret(&tmp);
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("保存失败：{err}"));
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
    #[cfg(all(target_os = "macos", not(test)))]
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
    #[cfg(any(not(target_os = "macos"), test))]
    {
        let _ = oauth;
        Ok(())
    }
}

fn keychain_read_oauth() -> Option<Value> {
    #[cfg(all(target_os = "macos", not(test)))]
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
    #[cfg(any(not(target_os = "macos"), test))]
    {
        None
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn keychain_delete() {
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", KEYCHAIN_SERVICE])
        .output();
}

fn run_claude(
    exe: &Path,
    args: &[&str],
    proxy: &[(String, String)],
    visible: bool,
) -> Result<String, String> {
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
        Err(format!(
            "{} 退出码 {}",
            exe.display(),
            output.status.code().unwrap_or(-1)
        ))
    }
}

fn apply_window(cmd: &mut Command, visible: bool) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        cmd.creation_flags(if visible {
            CREATE_NEW_CONSOLE
        } else {
            CREATE_NO_WINDOW
        });
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

    struct TestClient;

    impl OAuthClient for TestClient {
        fn profile(&self, oauth: &Value) -> Result<Value, AuthError> {
            let uuid = if claude_oauth::token(oauth, "accessToken")
                .is_some_and(|token| token.starts_with("tok-b"))
            {
                "acct-2"
            } else {
                "acct-1"
            };
            Ok(json!({ "account": { "uuid": uuid } }))
        }

        fn refresh(&self, _oauth: &Value) -> Result<Value, AuthError> {
            Err(AuthError::Rejected)
        }
    }

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
        write_live_oauth(
            &live,
            &json!({ "accessToken": "new", "subscriptionType": "pro" }),
        )
        .unwrap();
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
        let list = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
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
        save_current_in(&store, &live, "个人号".into(), &TestClient).unwrap();

        switch_account_in(&store, &live, &ada_id, &TestClient, || Ok(())).unwrap();
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
        save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
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
        let list = save_current_in(&store, &live, "公司号".into(), &TestClient).unwrap();
        assert_eq!(list.accounts.len(), 1);
        assert_eq!(list.accounts[0].name, "公司号");
        let snap = read_json(&snapshot_credentials(&store, &list.accounts[0].id).unwrap()).unwrap();
        assert_eq!(snap["claudeAiOauth"]["accessToken"], "tok-a2");
    }

    #[test]
    fn delete_snapshot_leaves_live() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let list = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
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
        let err = switch_account_in(&store, &live, "abc.json", &TestClient, || Ok(())).unwrap_err();
        assert_eq!(err, "账号编号不合法。");
    }

    #[test]
    fn listing_never_overwrites_a_snapshot_with_empty_or_mismatched_live_tokens() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        let path = snapshot_credentials(&store, &saved.accounts[0].id).unwrap();
        let original = fs::read(&path).unwrap();

        write_live_oauth(
            &live,
            &json!({"accessToken": "", "refreshToken": "", "expiresAt": 0}),
        )
        .unwrap();
        assert!(!list_accounts_in(&store, &live).unwrap().logged_in);
        assert!(save_current_in(&store, &live, String::new(), &TestClient).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);

        write_live_oauth(
            &live,
            &json!({"accessToken": "tok-b", "refreshToken": "tok-b-refresh"}),
        )
        .unwrap();
        list_accounts_in(&store, &live).unwrap();
        let err = save_current_in(&store, &live, String::new(), &TestClient).unwrap_err();
        assert!(err.contains("不属于同一账号"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn empty_target_is_rejected_without_stopping_terminals_or_changing_live_auth() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        let id = &saved.accounts[0].id;
        let path = snapshot_credentials(&store, id).unwrap();
        write_atomic(
            &path,
            br#"{"claudeAiOauth":{"accessToken":"","refreshToken":""}}"#,
            true,
        )
        .unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        let before = fs::read(&live.credentials).unwrap();
        let stopped = std::cell::Cell::new(false);
        let err = switch_account_in(&store, &live, id, &TestClient, || {
            stopped.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(err.contains("凭据为空"));
        assert!(!stopped.get());
        assert_eq!(fs::read(&live.credentials).unwrap(), before);
    }

    #[test]
    fn target_token_owner_mismatch_preserves_live_login_and_logs_no_secrets() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        let id = &saved.accounts[0].id;
        let path = snapshot_credentials(&store, id).unwrap();
        write_atomic(&path, br#"{"claudeAiOauth":{"accessToken":"tok-b-mismatched-secret","refreshToken":"refresh-secret"}}"#, true).unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        let before = fs::read(&live.credentials).unwrap();
        let config_before = fs::read(&live.claude_json).unwrap();
        let err = switch_account_in(&store, &live, id, &TestClient, || Ok(())).unwrap_err();
        assert!(err.contains("不属于同一账号"));
        assert_eq!(fs::read(&live.credentials).unwrap(), before);
        assert_eq!(fs::read(&live.claude_json).unwrap(), config_before);
        let log = fs::read_to_string(store.join("auth-diagnostics.jsonl")).unwrap();
        assert!(log.contains("identity_mismatch"));
        assert!(!log.contains("tok-b"));
        assert!(!log.contains("refresh-secret"));
        assert!(!log.contains("bob@example.com"));
    }

    #[test]
    fn rotated_token_survives_profile_failure_and_is_reused_on_retry() {
        use std::cell::Cell;
        struct RefreshClient {
            refreshes: Cell<u32>,
            fail_profile: Cell<bool>,
            stopped: Cell<bool>,
        }
        impl OAuthClient for RefreshClient {
            fn profile(&self, oauth: &Value) -> Result<Value, AuthError> {
                assert!(
                    self.stopped.get(),
                    "all verification happens after the old terminal exits"
                );
                if oauth["accessToken"] == "tok-a-new" && self.fail_profile.get() {
                    Err(AuthError::Network)
                } else {
                    TestClient.profile(oauth)
                }
            }
            fn refresh(&self, _oauth: &Value) -> Result<Value, AuthError> {
                assert!(self.stopped.get());
                self.refreshes.set(self.refreshes.get() + 1);
                Ok(
                    json!({"accessToken": "tok-a-new", "refreshToken": "rotated-secret", "expiresAt": Utc::now().timestamp_millis() + 3_600_000}),
                )
            }
        }
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        let id = &saved.accounts[0].id;
        let path = snapshot_credentials(&store, id).unwrap();
        let mut snapshot = read_json(&path).unwrap();
        snapshot["claudeAiOauth"]["expiresAt"] = json!(0);
        write_atomic(&path, serde_json::to_vec(&snapshot).unwrap(), true).unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        save_current_in(&store, &live, "个人号".into(), &TestClient).unwrap();
        let before = fs::read(&live.credentials).unwrap();
        let client = RefreshClient {
            refreshes: Cell::new(0),
            fail_profile: Cell::new(true),
            stopped: Cell::new(false),
        };
        let pending = path.with_file_name("pending-credentials.json");
        assert!(switch_account_in(&store, &live, id, &client, || {
            client.stopped.set(true);
            Ok(())
        })
        .is_err());
        assert_eq!(fs::read(&live.credentials).unwrap(), before);
        assert_eq!(
            read_json(&pending).unwrap()["claudeAiOauth"]["refreshToken"],
            "rotated-secret"
        );

        client.fail_profile.set(false);
        client.stopped.set(false);
        let result = switch_account_in(&store, &live, id, &client, || {
            client.stopped.set(true);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            client.refreshes.get(),
            1,
            "retry must not reuse the spent refresh token"
        );
        assert!(!pending.exists());
        assert_eq!(
            read_json(&live.credentials).unwrap()["claudeAiOauth"]["accessToken"],
            "tok-a-new"
        );
        assert_eq!(
            read_json(&live.claude_json).unwrap()["oauthAccount"]["accountUuid"],
            "acct-1"
        );
        assert!(result.accounts.iter().any(|a| a.name == "个人号"));
        assert_eq!(
            read_json(&live.credentials).unwrap()["mcpOAuth"]["keep"],
            true
        );
    }

    #[test]
    fn failed_write_restores_all_original_live_files() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        fs::write(&live.settings, b"invalid settings json").unwrap();
        let before: Vec<_> = [&live.credentials, &live.claude_json, &live.settings]
            .into_iter()
            .map(|path| fs::read(path).unwrap())
            .collect();
        let err = switch_account_in(&store, &live, &saved.accounts[0].id, &TestClient, || Ok(()))
            .unwrap_err();
        assert!(err.contains("损坏"));
        let after: Vec<_> = [&live.credentials, &live.claude_json, &live.settings]
            .into_iter()
            .map(|path| fs::read(path).unwrap())
            .collect();
        assert_eq!(after, before);
        assert!(fs::read_to_string(store.join("auth-diagnostics.jsonl"))
            .unwrap()
            .contains("rollback"));
    }

    #[test]
    fn failing_to_stop_old_terminals_cancels_the_switch() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        let before = fs::read(&live.credentials).unwrap();
        assert!(
            switch_account_in(&store, &live, &saved.accounts[0].id, &TestClient, || Err(
                "terminal still alive".into()
            ))
            .is_err()
        );
        assert_eq!(fs::read(&live.credentials).unwrap(), before);
    }

    #[test]
    fn manual_save_discards_stale_pending_tokens() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let saved = save_current_in(&store, &live, "工作号".into(), &TestClient).unwrap();
        let pending = snapshot_credentials(&store, &saved.accounts[0].id)
            .unwrap()
            .with_file_name("pending-credentials.json");
        fs::write(&pending, br#"{"claudeAiOauth":{"accessToken":"stale"}}"#).unwrap();
        save_current_in(&store, &live, String::new(), &TestClient).unwrap();
        assert!(!pending.exists());
        assert_eq!(
            list_accounts_in(&store, &live).unwrap().accounts[0].name,
            "工作号"
        );
    }

    #[test]
    fn early_server_rejection_refreshes_once_even_before_recorded_expiry() {
        struct EarlyRejection;
        impl OAuthClient for EarlyRejection {
            fn profile(&self, oauth: &Value) -> Result<Value, AuthError> {
                if oauth["accessToken"] == "old" {
                    Err(AuthError::Rejected)
                } else {
                    TestClient.profile(oauth)
                }
            }
            fn refresh(&self, _oauth: &Value) -> Result<Value, AuthError> {
                Ok(
                    json!({"accessToken": "new", "refreshToken": "rotated", "expiresAt": Utc::now().timestamp_millis() + 3_600_000}),
                )
            }
        }
        let (_root, store, _live) = fixture();
        let checkpoints = std::cell::Cell::new(0);
        let oauth = verified_oauth(&store,
            json!({"accessToken": "old", "refreshToken": "refresh", "expiresAt": Utc::now().timestamp_millis() + 3_600_000}),
            &json!({"accountUuid": "acct-1"}), &EarlyRejection,
            |_| { checkpoints.set(checkpoints.get() + 1); Ok(()) },
        ).unwrap();
        assert_eq!(oauth["accessToken"], "new");
        assert_eq!(checkpoints.get(), 1);
    }

    #[test]
    fn unverified_outgoing_login_is_backed_up_without_overwriting_its_account_snapshot() {
        let (_root, store, live) = fixture();
        seed_live(&live, "ada@example.com", "acct-1", "tok-a");
        let ada = save_current_in(&store, &live, "工作号".into(), &TestClient)
            .unwrap()
            .accounts[0]
            .id
            .clone();
        seed_live(&live, "bob@example.com", "acct-2", "tok-b");
        let bob = save_current_in(&store, &live, "个人号".into(), &TestClient)
            .unwrap()
            .accounts
            .into_iter()
            .find(|a| a.account_uuid == "acct-2")
            .unwrap()
            .id;
        let bob_path = snapshot_credentials(&store, &bob).unwrap();
        let original = fs::read(&bob_path).unwrap();
        write_live_oauth(
            &live,
            &json!({"accessToken": "tok-a", "refreshToken": "tok-a-refresh"}),
        )
        .unwrap();
        switch_account_in(&store, &live, &ada, &TestClient, || Ok(())).unwrap();
        assert_eq!(fs::read(&bob_path).unwrap(), original);
        let recovery = fs::read_dir(&store)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("unverified-live-")
            })
            .unwrap()
            .path();
        assert_eq!(
            read_json(&recovery).unwrap()["claudeAiOauth"]["accessToken"],
            "tok-a"
        );
        assert_eq!(
            read_json(&live.claude_json).unwrap()["oauthAccount"]["accountUuid"],
            "acct-1"
        );
    }
}
