//! Directory identities adapted from multi-claude's env.py, launcher.py and
//! identity.py. Attribution: THIRD_PARTY_NOTICES.md. Claude owns login, logout
//! and token refresh; Dock never swaps or copies live credentials.
use crate::claude_oauth::{self, HttpOAuthClient, OAuthClient};
use crate::{
    platform,
    state::AppSettings,
    tools::{self, ToolId},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const SYSTEM: &str = "system";
const AUTH_ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CLAUDE_SECURESTORAGE_CONFIG_DIR",
    "CLAUDE_CODE_CUSTOM_OAUTH_URL",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_HOME",
];
const PROXY_ENV: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
];
static OPERATION: Mutex<()> = Mutex::new(());
static STORE: OnceLock<PathBuf> = OnceLock::new();
static CANCEL_LOGIN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub account_uuid: String,
    pub subscription_type: String,
    pub updated_at: String,
    pub active: bool,
    pub config_dir: String,
    pub source: String,
    pub status: String,
    pub message: String,
    pub proxy: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountList {
    pub logged_in: bool,
    pub current_email: String,
    pub current_name: String,
    pub accounts: Vec<ClaudeAccount>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileIndex {
    #[serde(default)]
    default_account_id: Option<String>,
    #[serde(default)]
    accounts: Vec<Profile>,
    // Retain directory ownership for open terminals and history after removal.
    #[serde(default)]
    retired: Vec<Profile>,
    #[serde(default)]
    hidden_legacy: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    config_dir: PathBuf,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    explicit_config: bool,
    #[serde(default)]
    email: String,
    #[serde(default)]
    account_uuid: String,
    #[serde(default)]
    subscription_type: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default = "inherit_proxy")]
    proxy: String,
    #[serde(default)]
    verification_pending: bool,
    #[serde(default)]
    system_identity: Option<SystemIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemIdentity {
    access_token_hash: String,
    account_uuid: String,
    email: String,
}

fn inherit_proxy() -> String {
    "inherit".into()
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let _ = STORE.set(accounts_dir(app)?);
    Ok(())
}

pub fn lock_operation() -> Result<MutexGuard<'static, ()>, String> {
    OPERATION
        .try_lock()
        .map_err(|_| "Claude 账号正在处理中，请稍后重试。".into())
}

fn accounts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| "无法定位应用数据目录。")?
        .join("claude-accounts"))
}

fn system_profile() -> Profile {
    let configured = std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("CLAUDE_HOME").filter(|v| !v.is_empty()));
    Profile {
        id: SYSTEM.into(),
        name: "系统 Claude".into(),
        explicit_config: configured.is_some(),
        config_dir: tools::claude_home(),
        system: true,
        email: String::new(),
        account_uuid: String::new(),
        subscription_type: String::new(),
        updated_at: String::new(),
        proxy: inherit_proxy(),
        verification_pending: false,
        system_identity: None,
    }
}

fn config_file(profile: &Profile) -> PathBuf {
    if profile.explicit_config {
        profile.config_dir.join(".claude.json")
    } else {
        profile.config_dir.with_extension("json")
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|_| format!("无法读取 {}，请检查文件权限。", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| format!("{} 格式损坏，请修复后重试。", path.display()))
}

fn load_index(store: &Path) -> Result<ProfileIndex, String> {
    let path = store.join("profiles.json");
    if !path.exists() {
        return Ok(ProfileIndex::default());
    }
    serde_json::from_value(read_json(&path)?)
        .map_err(|_| "Claude 账号索引损坏，请检查 profiles.json。".into())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("文件路径不合法。")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建 Claude 账号目录。")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法设置账号目录权限。")?;
    }
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(value).map_err(|_| "无法保存 Claude 账号配置。")?;
    use std::io::Write;
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|_| "无法写入 Claude 账号配置。")?;
        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|_| "无法写入 Claude 账号配置。")?;
        fs::rename(&tmp, path).map_err(|_| "无法保存 Claude 账号配置，请检查文件权限。")
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(result?)
}

fn save_index(store: &Path, index: &ProfileIndex) -> Result<(), String> {
    write_json(&store.join("profiles.json"), index)
}

fn profiles(index: &ProfileIndex) -> Vec<Profile> {
    let mut out = vec![index
        .accounts
        .iter()
        .find(|p| p.id == SYSTEM)
        .cloned()
        .unwrap_or_else(system_profile)];
    out.extend(index.accounts.iter().filter(|p| p.id != SYSTEM).cloned());
    out
}

fn selected(index: &ProfileIndex) -> Result<Profile, String> {
    let id = index.default_account_id.as_deref().unwrap_or(SYSTEM);
    profiles(index)
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "默认 Claude 账号不存在，请重新选择。".into())
}

fn oauth_account(profile: &Profile) -> Option<Value> {
    read_json(&config_file(profile))
        .ok()?
        .get("oauthAccount")
        .filter(|v| v.is_object())
        .cloned()
}

fn credentials(profile: &Profile) -> Result<Option<Value>, String> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let service = keychain_service(profile);
        let output = Command::new("security")
            .args(["find-generic-password", "-s", &service, "-w"])
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|_| "无法读取 macOS Claude 钥匙串。")?;
        if output.status.success() {
            let root: Value =
                serde_json::from_slice(&output.stdout).map_err(|_| "Claude 钥匙串数据损坏。")?;
            return Ok(root.get("claudeAiOauth").cloned());
        }
        if output.status.code() != Some(44) {
            return Err("Claude 钥匙串不可用，请解锁后重试。".into());
        }
    }
    let path = profile.config_dir.join(".credentials.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(read_json(&path)?.get("claudeAiOauth").cloned())
}

#[cfg(target_os = "macos")]
fn keychain_service(profile: &Profile) -> String {
    use sha2::{Digest, Sha256};
    use unicode_normalization::UnicodeNormalization;
    let base = "Claude Code-credentials";
    if !profile.explicit_config {
        return base.into();
    }
    // Hash the exact launcher path (NFC), without resolving symlinks.
    let normalized: String = profile.config_dir.to_string_lossy().nfc().collect();
    format!(
        "{base}-{}",
        &format!("{:x}", Sha256::digest(normalized.as_bytes()))[..8]
    )
}

fn identity_matches(profile: &Profile, account: &Value) -> bool {
    // The system entry follows external CLI logins; its saved label must never
    // pin it to an old identity. Verify live metadata against the actual token.
    if profile.system {
        return true;
    }
    if !profile.account_uuid.is_empty() {
        account.get("accountUuid").and_then(Value::as_str) == Some(profile.account_uuid.as_str())
    } else if !profile.email.is_empty() {
        account
            .get("emailAddress")
            .and_then(Value::as_str)
            .is_some_and(|e| e.eq_ignore_ascii_case(&profile.email))
    } else {
        true
    }
}

fn token_hash(oauth: &Value) -> Option<String> {
    use sha2::{Digest, Sha256};
    let token = claude_oauth::token(oauth, "accessToken")?;
    Some(format!("{:x}", Sha256::digest(token.as_bytes())))
}

fn system_identity_problem(
    profile: &Profile,
    account: Option<&Value>,
    oauth: Option<&Value>,
) -> Option<(&'static str, String)> {
    if !profile.system {
        return None;
    }
    let oauth = oauth.filter(|v| claude_oauth::has_credentials(v))?;
    let checked = profile
        .system_identity
        .as_ref()
        .filter(|checked| token_hash(oauth).as_deref() == Some(checked.access_token_hash.as_str()));
    let Some(checked) = checked else {
        return Some((
            "unverified",
            "系统登录尚未核验，请点击「检查登录」。本地邮箱缓存不能证明凭证归属。".into(),
        ));
    };
    if account
        .and_then(|a| a.get("accountUuid"))
        .and_then(Value::as_str)
        != Some(checked.account_uuid.as_str())
    {
        let actual = checked
            .email
            .split_once('@')
            .map(|(name, domain)| {
                format!("{}***@{domain}", name.chars().take(2).collect::<String>())
            })
            .unwrap_or_else(|| "另一个账号".into());
        return Some(("conflict", format!("系统缓存身份与实际登录凭证不一致，凭证实际属于 {actual}。请在账号管理中点击「修复系统身份」，或在系统终端重新登录后再检查。")));
    }
    None
}

fn row(profile: &Profile, active: bool) -> ClaudeAccount {
    let account = oauth_account(profile);
    let oauth = credentials(profile);
    let (status, message) = if !profile.config_dir.is_dir() {
        (
            "broken",
            "账号目录不存在，请恢复目录或重新创建账号。".into(),
        )
    } else if account
        .as_ref()
        .is_some_and(|a| !identity_matches(profile, a))
    {
        (
            "conflict",
            "登录身份与此账号不符，请重新登录对应邮箱。".into(),
        )
    } else if account.is_none()
        && oauth
            .as_ref()
            .ok()
            .and_then(|v| v.as_ref())
            .is_some_and(claude_oauth::has_credentials)
    {
        (
            "unverified",
            "未找到此目录的账号身份，请重新登录以恢复身份信息。".into(),
        )
    } else if let Some(problem) = system_identity_problem(
        profile,
        account.as_ref(),
        oauth.as_ref().ok().and_then(|v| v.as_ref()),
    ) {
        problem
    } else if profile.verification_pending
        && oauth
            .as_ref()
            .ok()
            .and_then(|v| v.as_ref())
            .is_some_and(claude_oauth::has_credentials)
    {
        (
            "unverified",
            "登录尚未通过身份检查；如已完成授权，请点击「检查登录」。".into(),
        )
    } else if let Some(message) = auth_override(&[
        profile.config_dir.join("settings.json"),
        config_file(profile),
    ]) {
        ("conflict", message)
    } else {
        match &oauth {
            Err(err) => ("broken", err.clone()),
            Ok(Some(value)) if claude_oauth::has_credentials(value) => {
                if claude_oauth::needs_refresh(value) {
                    if claude_oauth::token(value, "refreshToken").is_some() {
                        (
                            "refreshable",
                            "访问令牌已到期；启动时由 Claude 自动续期。".into(),
                        )
                    } else {
                        ("signedOut", "登录已到期，请重新登录。".into())
                    }
                } else {
                    (
                        "ready",
                        "已检测到本地登录，实际可用性由 Claude 确认。".into(),
                    )
                }
            }
            _ => ("signedOut", "尚未登录此账号，请点击登录。".into()),
        }
    };
    let text = |key: &str| {
        account
            .as_ref()
            .and_then(|a| a.get(key))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    ClaudeAccount {
        id: profile.id.clone(),
        name: profile.name.clone(),
        email: if profile.system || profile.email.is_empty() {
            text("emailAddress")
        } else {
            profile.email.clone()
        },
        account_uuid: if profile.system || profile.account_uuid.is_empty() {
            text("accountUuid")
        } else {
            profile.account_uuid.clone()
        },
        subscription_type: oauth
            .ok()
            .flatten()
            .and_then(|v| {
                v.get("subscriptionType")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| profile.subscription_type.clone()),
        updated_at: profile.updated_at.clone(),
        active,
        config_dir: profile.config_dir.to_string_lossy().into_owned(),
        source: if profile.system { "system" } else { "isolated" }.into(),
        status: status.into(),
        message,
        proxy: profile.proxy.clone(),
    }
}

fn legacy_rows(store: &Path, index: &ProfileIndex) -> Result<Vec<ClaudeAccount>, String> {
    let path = store.join("index.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let legacy = read_json(&path)?;
    let entries = legacy
        .get("accounts")
        .and_then(Value::as_array)
        .ok_or("旧版 Claude 账号索引损坏。")?;
    let mut rows = Vec::new();
    for entry in entries {
        let text = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let id = text("id");
        validate_id(&id)?;
        if index
            .accounts
            .iter()
            .chain(&index.retired)
            .any(|p| p.id == id)
            || index.hidden_legacy.contains(&id)
        {
            continue;
        }
        rows.push(ClaudeAccount {
            id,
            name: text("name"),
            email: text("email"),
            account_uuid: text("accountUuid"),
            subscription_type: text("subscriptionType"),
            updated_at: text("updatedAt"),
            active: false,
            config_dir: String::new(),
            source: "legacy".into(),
            status: "legacy".into(),
            message: "旧版凭证快照。登录后转为独立账号，原快照保留。".into(),
            proxy: inherit_proxy(),
        });
    }
    Ok(rows)
}

fn list_in(store: &Path) -> Result<ClaudeAccountList, String> {
    let index = load_index(store)?;
    let current = selected(&index)?;
    let mut accounts: Vec<_> = profiles(&index)
        .iter()
        .map(|p| row(p, p.id == current.id))
        .collect();
    let current_row = accounts
        .iter()
        .find(|r| r.active)
        .expect("selected profile exists");
    let logged_in = matches!(current_row.status.as_str(), "ready" | "refreshable");
    let current_email = current_row.email.clone();
    let current_name = current_row.name.clone();
    accounts.extend(legacy_rows(store, &index)?);
    Ok(ClaudeAccountList {
        logged_in,
        current_email,
        current_name,
        accounts,
    })
}

pub fn list_accounts(app: &AppHandle) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    list_in(&accounts_dir(app)?)
}

fn validate_id(id: &str) -> Result<(), String> {
    if id != SYSTEM && Uuid::parse_str(id).is_err() {
        return Err("账号编号不合法。".into());
    }
    Ok(())
}

fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err("账号名称需要 1–40 个字符，不能包含控制字符。".into());
    }
    Ok(name.into())
}

fn normalize_proxy(proxy: &str) -> Result<String, String> {
    let proxy = proxy.trim();
    if matches!(proxy, "inherit" | "off") {
        return Ok(proxy.into());
    }
    let proxy = if proxy.bytes().all(|c| c.is_ascii_digit()) && !proxy.is_empty() {
        let port = proxy
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .ok_or("代理端口应为 1–65535。")?;
        format!("http://127.0.0.1:{port}")
    } else {
        proxy.to_string()
    };
    crate::proxy::validate_proxy_url(&proxy)?;
    ureq::Proxy::new(&proxy).map_err(|_| "代理地址无效，请填写 HTTP(S) URL 或端口。")?;
    let authority = proxy
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    if authority.is_empty() || authority.contains('@') || proxy.contains(['?', '#']) {
        return Err("代理需要 HTTP(S) 地址，不能包含用户名、密码或查询参数。".into());
    }
    Ok(proxy)
}

// Registration keeps the original directory by reference, including keychain.
pub fn save_current(app: &AppHandle, name: String) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let mut index = load_index(&store)?;
    let mut profile = index
        .accounts
        .iter()
        .find(|p| p.id == SYSTEM)
        .cloned()
        .unwrap_or_else(system_profile);
    profile.name = valid_name(&name)?;
    index.accounts.retain(|p| p.id != SYSTEM);
    index.accounts.push(profile);
    save_index(&store, &index)?;
    list_in(&store)
}

fn create_in(
    store: &Path,
    name: String,
    proxy: String,
    legacy: Option<ClaudeAccount>,
) -> Result<String, String> {
    let name = valid_name(&name)?;
    let proxy = normalize_proxy(&proxy)?;
    let mut index = load_index(store)?;
    let id = legacy
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_id(&id)?;
    if index.accounts.iter().any(|p| p.id == id) {
        return Err("这个账号已经创建，请直接登录。".into());
    }
    let config_dir = store.join("profiles").join(&id);
    fs::create_dir_all(config_dir.parent().ok_or("账号目录不合法。")?)
        .map_err(|_| "无法创建账号目录。")?;
    // Never silently take over a pre-existing directory.
    fs::create_dir(&config_dir).map_err(|_| "账号目录已存在或无法创建，请检查后重试。")?;
    if let Err(err) = write_json(&config_dir.join(".claude.json"), &json!({})) {
        let _ = fs::remove_dir(&config_dir);
        return Err(err);
    }
    index.accounts.push(Profile {
        id: id.clone(),
        name,
        config_dir: config_dir.clone(),
        explicit_config: true,
        system: false,
        email: legacy.as_ref().map(|a| a.email.clone()).unwrap_or_default(),
        account_uuid: legacy
            .as_ref()
            .map(|a| a.account_uuid.clone())
            .unwrap_or_default(),
        subscription_type: String::new(),
        updated_at: Utc::now().to_rfc3339(),
        proxy,
        verification_pending: false,
        system_identity: None,
    });
    if let Err(err) = save_index(store, &index) {
        // Only these freshly-created files belong to this attempt. Never
        // recursively remove a directory containing possible login/history.
        let _ = fs::remove_file(config_dir.join(".claude.json"));
        let _ = fs::remove_dir(&config_dir);
        return Err(err);
    }
    Ok(id)
}

pub fn create_account(
    app: &AppHandle,
    name: String,
    proxy: String,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    create_in(&store, name, proxy, None)?;
    list_in(&store)
}

pub fn update_account(
    app: &AppHandle,
    account_id: &str,
    name: String,
    proxy: String,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    validate_id(account_id)?;
    let store = accounts_dir(app)?;
    let mut index = load_index(&store)?;
    if account_id == SYSTEM && !index.accounts.iter().any(|p| p.id == SYSTEM) {
        index.accounts.push(system_profile());
    }
    let profile = index
        .accounts
        .iter_mut()
        .find(|p| p.id == account_id)
        .ok_or("没有找到此账号。")?;
    profile.name = valid_name(&name)?;
    profile.proxy = normalize_proxy(&proxy)?;
    save_index(&store, &index)?;
    list_in(&store)
}

fn switch_in(store: &Path, account_id: &str) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    let mut index = load_index(store)?;
    let profile = profiles(&index)
        .into_iter()
        .find(|p| p.id == account_id)
        .ok_or("没有找到此账号，请先创建或迁移。")?;
    let status = row(&profile, false);
    if !matches!(status.status.as_str(), "ready" | "refreshable") {
        return Err(status.message);
    }
    index.default_account_id = Some(account_id.into());
    save_index(store, &index)?;
    list_in(store)
}

pub fn switch_account(app: &AppHandle, account_id: &str) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let index = load_index(&store)?;
    if let Some(profile) = profiles(&index)
        .into_iter()
        .find(|p| p.id == account_id && p.system)
    {
        let settings = crate::state::load_state(app)?.settings;
        let mut base: BTreeMap<_, _> = std::env::vars().collect();
        base.extend(crate::proxy::proxy_env(
            !settings.default_proxy_url.trim().is_empty(),
            &settings.default_proxy_url,
        )?);
        prepare_system(&store, profile, &base)?;
    }
    switch_in(&store, account_id)
}

fn delete_in(store: &Path, account_id: &str) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    if account_id == SYSTEM {
        return Err("系统 Claude 入口需要保留，可以修改名称和代理。".into());
    }
    let mut index = load_index(store)?;
    if let Some(pos) = index.accounts.iter().position(|p| p.id == account_id) {
        let removed = index.accounts.remove(pos);
        index.retired.push(removed);
    } else if !legacy_rows(store, &index)?
        .iter()
        .any(|p| p.id == account_id)
    {
        return Err("没有找到此账号。".into());
    }
    if !index.hidden_legacy.iter().any(|id| id == account_id) {
        index.hidden_legacy.push(account_id.into());
    }
    if index.default_account_id.as_deref() == Some(account_id) {
        index.default_account_id = None;
    }
    save_index(store, &index)?;
    list_in(store)
}

pub fn delete_account(app: &AppHandle, account_id: &str) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    delete_in(&accounts_dir(app)?, account_id)
}

pub fn clear_credentials(
    app: &AppHandle, settings: &AppSettings, account_id: &str,
    stop_terminals: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let result = clear_credentials_in(&store, account_id, stop_terminals, |profile| logout_cli(profile, settings));
    action_log(&store, "clear_credentials", json!({"accountId": account_id, "ok": result.is_ok()}));
    result
}

fn clear_credentials_in(
    store: &Path, account_id: &str,
    stop_terminals: impl FnOnce() -> Result<(), String>,
    logout: impl FnOnce(&Profile) -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    validate_id(account_id)?;
    let mut index = load_index(store)?;
    if let Some(mut profile) = profiles(&index).into_iter().find(|p| p.id == account_id) {
        stop_terminals()?;
        logout(&profile)?;
        if credentials(&profile)?.as_ref().is_some_and(claude_oauth::has_credentials) {
            return Err("仍检测到登录凭据，请关闭使用该目录的其他 Claude 窗口后重试。".into());
        }
        clear_legacy_credentials(store, account_id)?;
        profile.system_identity = None;
        profile.verification_pending = false;
        profile.subscription_type.clear();
        profile.updated_at = Utc::now().to_rfc3339();
        index.accounts.retain(|p| p.id != account_id);
        index.accounts.push(profile);
        save_index(store, &index)?;
    } else {
        if !legacy_rows(store, &index)?.iter().any(|p| p.id == account_id) {
            return Err("没有找到此账号。".into());
        }
        clear_legacy_credentials(store, account_id)?;
    }
    list_in(store)
}

fn clear_legacy_credentials(store: &Path, account_id: &str) -> Result<(), String> {
    if account_id == SYSTEM { return Ok(()); }
    for filename in ["credentials.json", "pending-credentials.json"] {
        let path = store.join(account_id).join(filename);
        if path.is_file() {
            let mut root = read_json(&path)?;
            root.as_object_mut().ok_or("旧版凭据格式损坏，无法清理。")?.remove("claudeAiOauth");
            write_json(&path, &root)?;
        }
    }
    Ok(())
}

fn logout_cli(profile: &Profile, settings: &AppSettings) -> Result<(), String> {
    let exe = tools::resolve_binary(ToolId::Claude, settings)?;
    let mut base: BTreeMap<_, _> = std::env::vars().collect();
    base.extend(crate::proxy::proxy_env(!settings.default_proxy_url.trim().is_empty(), &settings.default_proxy_url)?);
    let context = launch_env(profile, &base)?;
    let mut cmd = Command::new(exe);
    cmd.args(["auth", "logout"]).current_dir(&profile.config_dir);
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    for (key, value) in base.iter().filter(|(key, _)| PROXY_ENV.contains(&key.as_str())) { cmd.env(key, value); }
    for key in &context.remove { cmd.env_remove(key); }
    cmd.envs(&context.set);
    let mut child = cmd.spawn().map_err(|_| "无法启动 Claude 退出登录，请检查 CLI 路径。")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait().map_err(|_| "无法读取退出登录状态。")? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => return Err("Claude 未能清除登录，请关闭使用该目录的其他窗口后重试。".into()),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("退出登录超时，请检查网络后重试。".into());
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionAccounts {
    pub source_account_id: String,
    pub source_account_name: String,
    pub accounts: Vec<ClaudeAccount>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeContinuation {
    pub session_id: String,
    pub title: String,
    pub account_id: String,
    pub account_name: String,
}

pub fn session_accounts(app: &AppHandle, cwd: &str, session_id: &str) -> Result<ClaudeSessionAccounts, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let source = owner_in(&store, cwd, Some(session_id))?;
    Ok(ClaudeSessionAccounts {
        source_account_id: source.id, source_account_name: source.name,
        accounts: list_in(&store)?.accounts,
    })
}

pub fn continue_with_account(app: &AppHandle, cwd: &str, session_id: &str, account_id: &str, title: &str) -> Result<ClaudeContinuation, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let result = continue_in(&store, cwd, session_id, account_id, title);
    action_log(&store, "continue_with_account", json!({"accountId": account_id, "ok": result.is_ok()}));
    result
}

fn continue_in(store: &Path, cwd: &str, session_id: &str, account_id: &str, title: &str) -> Result<ClaudeContinuation, String> {
    validate_id(account_id)?;
    let source = owner_in(store, cwd, Some(session_id))?;
    let index = load_index(store)?;
    let target = profiles(&index).into_iter().find(|p| p.id == account_id).ok_or("目标账号不存在，请刷新账号列表。")?;
    let status = row(&target, false);
    if !matches!(status.status.as_str(), "ready" | "refreshable") { return Err(status.message); }
    let title: String = title.trim().chars().filter(|c| !c.is_control()).take(120).collect();
    let title = format!("{} · 续接", if title.is_empty() { "Claude 会话" } else { &title });
    let copied = crate::claude_session_copy::copy_session(&source.config_dir, &target.config_dir, cwd, session_id, &title)?;
    record_copied_session(store, &copied);
    Ok(ClaudeContinuation { session_id: copied, title, account_id: target.id, account_name: target.name })
}

pub fn copy_ledger_file() -> Option<PathBuf> {
    STORE.get().map(|store| store.join("copy-ledger.json"))
}

fn record_copied_session(store: &Path, session_id: &str) {
    let path = store.join("copy-ledger.json");
    let mut map: HashMap<String, i64> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    map.insert(session_id.to_string(), Utc::now().timestamp());
    if let Ok(bytes) = serde_json::to_vec_pretty(&map) {
        let _ = fs::write(path, bytes);
    }
}

fn action_log(store: &Path, event: &str, details: Value) {
    use std::io::Write;
    let _ = (|| -> std::io::Result<()> {
        fs::create_dir_all(store)?;
        let path = store.join("auth-diagnostics.jsonl");
        if fs::metadata(&path).is_ok_and(|m| m.len() > 1_048_576) {
            fs::rename(&path, store.join("auth-diagnostics.previous.jsonl"))?;
        }
        let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", json!({"at": Utc::now().to_rfc3339(), "event": event, "details": details}))
    })();
}

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub id: String,
    pub name: String,
    pub set: BTreeMap<String, String>,
    pub remove: Vec<String>,
}

fn launch_env(profile: &Profile, base: &BTreeMap<String, String>) -> Result<LaunchContext, String> {
    let mut ctx = LaunchContext {
        id: profile.id.clone(),
        name: profile.name.clone(),
        set: BTreeMap::new(),
        remove: AUTH_ENV.iter().map(|s| s.to_string()).collect(),
    };
    if profile.explicit_config {
        ctx.set.insert(
            "CLAUDE_CONFIG_DIR".into(),
            profile.config_dir.to_string_lossy().into_owned(),
        );
    }
    match profile.proxy.as_str() {
        "inherit" => (),
        "off" => {
            ctx.remove.extend(PROXY_ENV.iter().map(|s| s.to_string()));
            ctx.remove.extend([
                "CLAUDE_CODE_HTTP_PROXY".into(),
                "CLAUDE_CODE_HTTPS_PROXY".into(),
            ]);
        }
        proxy => {
            let proxy = normalize_proxy(proxy)?;
            for key in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
                ctx.set.insert(key.into(), proxy.clone());
            }
            ctx.remove.extend([
                "ALL_PROXY".into(),
                "all_proxy".into(),
                "CLAUDE_CODE_HTTP_PROXY".into(),
                "CLAUDE_CODE_HTTPS_PROXY".into(),
            ]);
            for key in ["NO_PROXY", "no_proxy"] {
                let prefix = base
                    .get(key)
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{s},"))
                    .unwrap_or_default();
                ctx.set
                    .insert(key.into(), format!("{prefix}localhost,127.0.0.1,::1"));
            }
        }
    }
    Ok(ctx)
}

fn owner_in(store: &Path, cwd: &str, session_id: Option<&str>) -> Result<Profile, String> {
    let index = load_index(store)?;
    if let Some(id) = session_id {
        let mut owners = Vec::new();
        for p in profiles(&index)
            .into_iter()
            .chain(index.retired.iter().cloned())
        {
            if crate::tools::claude::contains_session_in(&p.config_dir, cwd, id) {
                owners.push(p);
            }
        }
        if owners.len() > 1 {
            return Err("此会话存在于多个账号目录，请先处理重复的会话文件。".into());
        }
        return owners
            .pop()
            .ok_or_else(|| "未找到此会话所属的 Claude 账号目录，无法恢复。".into());
    }
    selected(&index)
}

pub fn launch_context(
    app: &AppHandle,
    cwd: &str,
    session_id: Option<&str>,
    base: &BTreeMap<String, String>,
) -> Result<LaunchContext, String> {
    let store = accounts_dir(app)?;
    let mut owner = owner_in(&store, cwd, session_id)?;
    let provider_override =
        auth_override(&[owner.config_dir.join("settings.json"), config_file(&owner)]);
    if owner.system && provider_override.is_none() {
        owner = prepare_system(&store, owner, base)?;
    }
    if !owner.config_dir.is_dir() && !owner.system {
        return Err("Claude 账号目录不存在，请先恢复目录。".into());
    }
    let status = row(&owner, false);
    if !owner.system {
        if matches!(status.status.as_str(), "conflict" | "unverified") {
            return Err(status.message);
        }
        if let Some(message) = auth_override(&[
            Path::new(cwd).join(".claude/settings.json"),
            Path::new(cwd).join(".claude/settings.local.json"),
        ]) {
            return Err(format!("项目设置覆盖了账号登录。{message}"));
        }
    }
    launch_env(&owner, base)
}

fn prepare_system(
    store: &Path,
    mut profile: Profile,
    base: &BTreeMap<String, String>,
) -> Result<Profile, String> {
    let status = row(&profile, false);
    if status.status == "unverified" {
        let mut index = load_index(store)?;
        verify_in(
            store,
            &mut index,
            &mut profile,
            &AppSettings::default(),
            base,
        )?;
    }
    let status = row(&profile, false);
    if matches!(status.status.as_str(), "conflict" | "unverified") {
        return Err(status.message);
    }
    Ok(profile)
}

pub fn session_homes() -> Vec<PathBuf> {
    let mut homes = vec![system_profile().config_dir];
    if let Some(store) = STORE.get() {
        if let Ok(index) = load_index(store) {
            for p in profiles(&index).into_iter().chain(index.retired) {
                if !homes.contains(&p.config_dir) {
                    homes.push(p.config_dir);
                }
            }
        }
    }
    homes
}

pub fn extra_usage_profiles() -> Vec<crate::usage::ExtraClaudeProfile> {
    let Some(store) = STORE.get() else {
        return Vec::new();
    };
    let Ok(index) = load_index(store) else {
        return Vec::new();
    };
    index
        .accounts
        .iter()
        .chain(index.retired.iter())
        .filter(|p| !p.system && p.id != SYSTEM)
        .map(|p| crate::usage::ExtraClaudeProfile {
            id: p.id.clone(),
            name: p.name.clone(),
            email: p.email.clone(),
            config_dir: p.config_dir.clone(),
        })
        .collect()
}

pub fn login_account(
    app: &AppHandle,
    settings: &AppSettings,
    account_id: &str,
    ensure_idle: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    CANCEL_LOGIN.store(false, Ordering::SeqCst);
    ensure_idle()?;
    let store = accounts_dir(app)?;
    let mut index = load_index(&store)?;
    validate_id(account_id)?;
    if !profiles(&index).iter().any(|p| p.id == account_id) {
        let legacy = legacy_rows(&store, &index)?
            .into_iter()
            .find(|p| p.id == account_id)
            .ok_or("没有找到此账号。")?;
        create_in(
            &store,
            legacy.name.clone(),
            legacy.proxy.clone(),
            Some(legacy),
        )?;
        index = load_index(&store)?;
    }
    let mut profile = profiles(&index)
        .into_iter()
        .find(|p| p.id == account_id)
        .ok_or("没有找到此账号。")?;
    if profile.system {
        return Err(
            "请在系统终端运行 claude auth login；新账号请使用「添加账号」创建独立目录。".into(),
        );
    }
    let exe = tools::resolve_binary(ToolId::Claude, settings)?;
    profile.verification_pending = true;
    index.accounts.retain(|p| p.id != profile.id);
    index.accounts.push(profile.clone());
    save_index(&store, &index)?;
    let mut base: BTreeMap<_, _> = std::env::vars().collect();
    for (k, v) in crate::proxy::proxy_env(
        !settings.default_proxy_url.trim().is_empty(),
        &settings.default_proxy_url,
    )? {
        base.insert(k, v);
    }
    let context = launch_env(&profile, &base)?;
    let mut cmd = Command::new(&exe);
    cmd.args(["auth", "login", "--claudeai"])
        .current_dir(&profile.config_dir);
    platform::prepare_command(&mut cmd);
    for (k, v) in base.iter().filter(|(k, _)| PROXY_ENV.contains(&k.as_str())) {
        cmd.env(k, v);
    }
    for key in &context.remove {
        cmd.env_remove(key);
    }
    cmd.envs(&context.set);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0010); // interactive login explicitly requested
    }
    // Never return raw CLI output containing auth URLs/codes or token material.
    let mut child = cmd
        .spawn()
        .map_err(|_| "无法启动 Claude 登录，请检查 CLI 路径。")?;
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if CANCEL_LOGIN.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("已取消 Claude 登录，账号目录已保留，可稍后重试。".into());
        }
        match child.try_wait().map_err(|_| "无法读取 Claude 登录状态。")? {
            Some(status) if status.success() => break,
            Some(_) => return Err("Claude 登录未完成。请重新登录，并在浏览器中完成授权。".into()),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Claude 登录等待超过 5 分钟，请重新登录。".into());
            }
            None => std::thread::sleep(Duration::from_millis(200)),
        }
    }
    verify_in(&store, &mut index, &mut profile, settings, &base)?;
    list_in(&store)
}

pub fn cancel_login() {
    CANCEL_LOGIN.store(true, Ordering::SeqCst);
}

fn auth_override(paths: &[PathBuf]) -> Option<String> {
    for path in paths {
        if !path.exists() {
            continue;
        }
        let root = match read_json(path) {
            Ok(root) => root,
            Err(err) => return Some(err),
        };
        let keys: Vec<_> = root
            .get("env")
            .and_then(Value::as_object)
            .map(|env| {
                AUTH_ENV
                    .iter()
                    .filter(|key| {
                        env.get(**key)
                            .is_some_and(|v| v.as_str().is_none_or(|s| !s.is_empty()))
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        if !keys.is_empty() || root.get("apiKeyHelper").is_some_and(|v| !v.is_null()) {
            return Some(format!(
                "{} 中的认证配置会覆盖账号登录，请移除对应配置后重试（{}）。",
                path.display(),
                if keys.is_empty() {
                    "apiKeyHelper".into()
                } else {
                    keys.join(", ")
                }
            ));
        }
    }
    None
}

fn verify_in(
    store: &Path,
    index: &mut ProfileIndex,
    profile: &mut Profile,
    settings: &AppSettings,
    base: &BTreeMap<String, String>,
) -> Result<(), String> {
    let client = client_for(profile, settings, base)?;
    verify_with_client(store, index, profile, &client)
}

fn client_for(profile: &Profile, settings: &AppSettings, base: &BTreeMap<String, String>) -> Result<HttpOAuthClient, String> {
    let mut auth_settings = settings.clone();
    auth_settings.default_proxy_url = match profile.proxy.as_str() {
        "inherit" => base
            .get("HTTPS_PROXY")
            .or_else(|| base.get("https_proxy"))
            .cloned()
            .unwrap_or_default(),
        "off" => String::new(),
        proxy => proxy.into(),
    };
    HttpOAuthClient::new(&auth_settings)
}

fn verify_with_client(
    store: &Path,
    index: &mut ProfileIndex,
    profile: &mut Profile,
    client: &impl OAuthClient,
) -> Result<(), String> {
    let oauth = credentials(profile)?
        .filter(claude_oauth::has_credentials)
        .ok_or("Claude 没有保存登录凭据，请重新登录。")?;
    let account = oauth_account(profile).ok_or("Claude 没有保存账号身份，请重新登录。")?;
    if !identity_matches(profile, &account) {
        return Err("登录邮箱与此账号不符，请使用对应的邮箱重新登录。".into());
    }
    let actual = client.profile(&oauth).map_err(|e| e.to_string())?;
    let identity_result = claude_oauth::verify_identity(&account, &actual);
    if profile.system {
        let actual_account = actual
            .get("account")
            .ok_or_else(|| claude_oauth::AuthError::InvalidResponse.to_string())?;
        profile.system_identity = Some(SystemIdentity {
            access_token_hash: token_hash(&oauth)
                .ok_or_else(|| claude_oauth::AuthError::Empty.to_string())?,
            account_uuid: claude_oauth::token(actual_account, "uuid")
                .ok_or_else(|| claude_oauth::AuthError::InvalidResponse.to_string())?
                .into(),
            email: actual_account
                .get("email")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
        });
        if let Err(err) = identity_result {
            profile.verification_pending = true;
            index.accounts.retain(|p| p.id != profile.id);
            index.accounts.push(profile.clone());
            save_index(store, index)?;
            return Err(format!("{} {}", err, row(profile, false).message));
        }
    } else {
        identity_result.map_err(|e| e.to_string())?;
        profile.email = account
            .get("emailAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into();
        profile.account_uuid = account
            .get("accountUuid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into();
    }
    profile.subscription_type = oauth
        .get("subscriptionType")
        .and_then(Value::as_str)
        .unwrap_or("")
        .into();
    profile.updated_at = Utc::now().to_rfc3339();
    index.accounts.retain(|p| p.id != profile.id);
    profile.verification_pending = false;
    index.accounts.push(profile.clone());
    save_index(store, index)
}

pub fn verify_account(
    app: &AppHandle,
    settings: &AppSettings,
    account_id: &str,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let mut index = load_index(&store)?;
    let mut profile = profiles(&index)
        .into_iter()
        .find(|p| p.id == account_id)
        .ok_or("没有找到此账号。")?;
    profile.verification_pending = true;
    index.accounts.retain(|p| p.id != profile.id);
    index.accounts.push(profile.clone());
    save_index(&store, &index)?;
    let mut base: BTreeMap<_, _> = std::env::vars().collect();
    base.extend(crate::proxy::proxy_env(
        !settings.default_proxy_url.trim().is_empty(),
        &settings.default_proxy_url,
    )?);
    verify_in(&store, &mut index, &mut profile, settings, &base)?;
    list_in(&store)
}

pub fn repair_system_identity(
    app: &AppHandle, settings: &AppSettings, stop_terminals: impl FnOnce() -> Result<(), String>,
) -> Result<ClaudeAccountList, String> {
    let _guard = lock_operation()?;
    let store = accounts_dir(app)?;
    let mut index = load_index(&store)?;
    let mut profile = profiles(&index).into_iter().find(|p| p.system).ok_or("系统 Claude 入口不存在。")?;
    stop_terminals()?;
    let mut base: BTreeMap<_, _> = std::env::vars().collect();
    base.extend(crate::proxy::proxy_env(!settings.default_proxy_url.trim().is_empty(), &settings.default_proxy_url)?);
    let client = client_for(&profile, settings, &base)?;
    let result = repair_system_in(&store, &mut index, &mut profile, &client);
    action_log(&store, "repair_system_identity", json!({"ok": result.is_ok()}));
    result?;
    list_in(&store)
}

fn repair_system_in(store: &Path, index: &mut ProfileIndex, profile: &mut Profile, client: &impl OAuthClient) -> Result<(), String> {
    if !profile.system { return Err("仅系统 Claude 支持同步系统身份缓存。".into()); }
    if let Some(message) = auth_override(&[profile.config_dir.join("settings.json"), config_file(profile)]) {
        return Err(message);
    }
    let oauth = credentials(profile)?.filter(claude_oauth::has_credentials)
        .ok_or("系统 Claude 尚未登录，请先在系统终端登录。")?;
    let hash = token_hash(&oauth).ok_or("系统访问令牌不可用，请先在系统终端重新登录。")?;
    let actual = client.profile(&oauth).map_err(|e| e.to_string())?;
    let account = actual.get("account").and_then(Value::as_object).ok_or("身份接口缺少账号信息，未修改系统缓存。")?;
    let uuid = account.get("uuid").and_then(Value::as_str).filter(|s| !s.trim().is_empty())
        .ok_or("身份接口缺少账号编号，未修改系统缓存。")?;
    let email = account.get("email").and_then(Value::as_str).filter(|s| !s.trim().is_empty())
        .ok_or("身份接口缺少邮箱，未修改系统缓存。")?;
    let path = config_file(profile);
    let mut root = if path.exists() { read_json(&path)? } else { json!({}) };
    let object = root.as_object_mut().ok_or("系统配置格式损坏，无法修复身份。")?;
    let mut metadata = object.get("oauthAccount")
        .filter(|a| a.get("accountUuid").and_then(Value::as_str) == Some(uuid))
        .cloned().unwrap_or_else(|| json!({}));
    metadata["accountUuid"] = json!(uuid);
    metadata["emailAddress"] = json!(email);
    metadata["profileFetchedAt"] = json!(Utc::now().timestamp_millis());
    for (target, source) in [("displayName", "display_name"), ("fullName", "full_name"), ("accountCreatedAt", "created_at")] {
        if let Some(value) = account.get(source) { metadata[target] = value.clone(); }
    }
    if let Some(organization) = actual.get("organization").and_then(Value::as_object) {
        for (target, source) in [("organizationUuid", "uuid"), ("organizationName", "name"), ("billingType", "billing_type")] {
            if let Some(value) = organization.get(source) { metadata[target] = value.clone(); }
        }
    }
    object.insert("oauthAccount".into(), metadata);
    if credentials(profile)?.as_ref().and_then(token_hash).as_deref() != Some(hash.as_str()) {
        return Err("核验期间系统登录已变化，请关闭使用系统目录的其他 Claude 窗口后重试。".into());
    }
    profile.system_identity = Some(SystemIdentity { access_token_hash: hash, account_uuid: uuid.into(), email: email.into() });
    profile.verification_pending = true;
    index.accounts.retain(|p| p.id != profile.id);
    index.accounts.push(profile.clone());
    save_index(store, index)?;
    write_json(&path, &root)?;
    profile.verification_pending = false;
    profile.updated_at = Utc::now().to_rfc3339();
    index.accounts.retain(|p| p.id != profile.id);
    index.accounts.push(profile.clone());
    save_index(store, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude_oauth::AuthError;

    fn fixture() -> (tempfile::TempDir, Profile) {
        let tmp = tempfile::tempdir().unwrap();
        let id = create_in(tmp.path(), "工作账号".into(), "inherit".into(), None).unwrap();
        let profile = load_index(tmp.path())
            .unwrap()
            .accounts
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        (tmp, profile)
    }

    fn logged_in(p: &Profile, expiry: i64) {
        write_json(&p.config_dir.join(".credentials.json"), &json!({"claudeAiOauth": {
            "accessToken": "test-access-secret", "refreshToken": "test-refresh-secret", "expiresAt": expiry, "subscriptionType": "pro"
        }})).unwrap();
        write_json(&config_file(p), &json!({"oauthAccount": {"accountUuid": "work-id", "emailAddress": "work@example.com"}})).unwrap();
    }

    struct FakeClient(Result<Value, AuthError>);
    impl OAuthClient for FakeClient {
        fn profile(&self, _: &Value) -> Result<Value, AuthError> {
            self.0.clone()
        }
    }

    #[test]
    fn new_profiles_have_separate_empty_directories_and_private_settings() {
        let (tmp, first) = fixture();
        let id = create_in(tmp.path(), "个人账号".into(), "7891".into(), None).unwrap();
        let index = load_index(tmp.path()).unwrap();
        let second = index.accounts.iter().find(|p| p.id == id).unwrap();
        assert_ne!(first.config_dir, second.config_dir);
        assert_eq!(second.proxy, "http://127.0.0.1:7891");
        assert!(!first.config_dir.join(".credentials.json").exists());
        assert!(!second.config_dir.join("settings.json").exists());
        assert_eq!(row(&first, false).status, "signedOut");
    }

    #[test]
    fn launch_environment_removes_credentials_and_keeps_identity_local() {
        let (_, first) = fixture();
        let base = BTreeMap::from([
            ("ANTHROPIC_API_KEY".into(), "global-secret".into()),
            ("HTTPS_PROXY".into(), "http://127.0.0.1:7890".into()),
        ]);
        let ctx = launch_env(&first, &base).unwrap();
        for key in AUTH_ENV {
            assert!(ctx.remove.contains(&key.to_string()));
        }
        assert_eq!(
            ctx.set["CLAUDE_CONFIG_DIR"],
            first.config_dir.to_string_lossy()
        );
        assert!(!ctx.set.values().any(|v| v.contains("global-secret")));
        assert!(!ctx.set.contains_key("HTTPS_PROXY"));
        let default = launch_env(&system_profile(), &base).unwrap();
        if !system_profile().explicit_config {
            assert!(!default.set.contains_key("CLAUDE_CONFIG_DIR"));
        }
    }

    #[test]
    fn proxies_override_project_proxy_and_off_removes_all_inherited_variants() {
        let (_, mut p) = fixture();
        p.proxy = "http://127.0.0.1:7901".into();
        let base = BTreeMap::from([("NO_PROXY".into(), "internal.example".into())]);
        let ctx = launch_env(&p, &base).unwrap();
        assert_eq!(ctx.set["HTTPS_PROXY"], p.proxy);
        assert_eq!(
            ctx.set["NO_PROXY"],
            "internal.example,localhost,127.0.0.1,::1"
        );
        assert!(ctx.remove.contains(&"ALL_PROXY".into()));
        p.proxy = "off".into();
        let off = launch_env(&p, &base).unwrap();
        for key in PROXY_ENV {
            assert!(off.remove.contains(&key.to_string()));
        }
        assert!(off.remove.contains(&"CLAUDE_CODE_HTTPS_PROXY".into()));
        assert!(!off.set.contains_key("HTTPS_PROXY"));
    }

    #[test]
    fn proxy_validation_rejects_secrets_socks_and_invalid_ports() {
        for proxy in [
            "0",
            "65536",
            "socks5://127.0.0.1:7890",
            "http://user:secret@host:80",
            "http://host:80?token=secret",
            "http://",
            "http://host\n",
        ] {
            // Surrounding whitespace is trimmed; embedded control chars are not.
            if proxy == "http://host\n" {
                continue;
            }
            assert!(normalize_proxy(proxy).is_err(), "accepted {proxy}");
        }
        assert!(normalize_proxy("http://ho\nst:80").is_err());
        assert_eq!(normalize_proxy("7901").unwrap(), "http://127.0.0.1:7901");
    }

    #[test]
    fn listing_is_read_only_and_expired_tokens_are_refreshable() {
        let (tmp, p) = fixture();
        logged_in(&p, 0);
        let path = p.config_dir.join(".credentials.json");
        let before = fs::read(&path).unwrap();
        assert_eq!(
            list_in(tmp.path())
                .unwrap()
                .accounts
                .iter()
                .find(|r| r.id == p.id)
                .unwrap()
                .status,
            "refreshable"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!fs::read_to_string(tmp.path().join("profiles.json"))
            .unwrap()
            .contains("test-refresh-secret"));
    }

    #[test]
    fn switching_only_updates_default_and_does_not_write_either_identity() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        let id = create_in(tmp.path(), "个人".into(), "off".into(), None).unwrap();
        let second = load_index(tmp.path())
            .unwrap()
            .accounts
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        logged_in(&second, i64::MAX);
        let first_bytes = fs::read(p.config_dir.join(".credentials.json")).unwrap();
        let second_bytes = fs::read(second.config_dir.join(".credentials.json")).unwrap();
        switch_in(tmp.path(), &p.id).unwrap();
        switch_in(tmp.path(), &second.id).unwrap();
        assert_eq!(
            selected(&load_index(tmp.path()).unwrap()).unwrap().id,
            second.id
        );
        assert_eq!(
            fs::read(p.config_dir.join(".credentials.json")).unwrap(),
            first_bytes
        );
        assert_eq!(
            fs::read(second.config_dir.join(".credentials.json")).unwrap(),
            second_bytes
        );
    }

    #[test]
    fn empty_credentials_and_wrong_identity_cannot_be_selected() {
        let (tmp, mut p) = fixture();
        assert!(switch_in(tmp.path(), &p.id).is_err());
        logged_in(&p, i64::MAX);
        p.account_uuid = "another-account".into();
        save_index(
            tmp.path(),
            &ProfileIndex {
                accounts: vec![p.clone()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(row(&p, false).status, "conflict");
        assert!(switch_in(tmp.path(), &p.id).is_err());
    }

    #[test]
    fn credentials_without_account_metadata_are_not_reported_as_ready() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        fs::remove_file(config_file(&p)).unwrap();
        assert_eq!(row(&p, false).status, "unverified");
        assert!(switch_in(tmp.path(), &p.id).is_err());
    }

    #[test]
    fn duplicate_session_ids_are_rejected_instead_of_using_default_account() {
        let (tmp, p) = fixture();
        let cwd = "D:/project";
        let id = create_in(tmp.path(), "个人".into(), "off".into(), None).unwrap();
        let index = load_index(tmp.path()).unwrap();
        let second = index.accounts.iter().find(|a| a.id == id).unwrap();
        for profile in [&p, second] {
            let project = profile
                .config_dir
                .join("projects")
                .join(crate::tools::claude::encode_project_dir(cwd));
            fs::create_dir_all(&project).unwrap();
            fs::write(project.join("same-id.jsonl"), "").unwrap();
        }
        assert!(owner_in(tmp.path(), cwd, Some("same-id")).is_err());
    }

    #[test]
    fn server_verification_failure_never_marks_login_ready_or_copies_tokens() {
        let (tmp, mut p) = fixture();
        logged_in(&p, i64::MAX);
        p.verification_pending = true;
        let mut index = ProfileIndex {
            accounts: vec![p.clone()],
            ..Default::default()
        };
        save_index(tmp.path(), &index).unwrap();
        let before = fs::read(p.config_dir.join(".credentials.json")).unwrap();
        for result in [
            Err(AuthError::Network),
            Ok(json!({"account": {"uuid": "wrong-id"}})),
        ] {
            assert!(
                verify_with_client(tmp.path(), &mut index, &mut p, &FakeClient(result)).is_err()
            );
            assert_eq!(row(&p, false).status, "unverified");
            assert!(switch_in(tmp.path(), &p.id).is_err());
        }
        verify_with_client(
            tmp.path(),
            &mut index,
            &mut p,
            &FakeClient(Ok(json!({"account": {"uuid": "work-id"}}))),
        )
        .unwrap();
        assert_eq!(row(&p, false).status, "ready");
        assert_eq!(p.account_uuid, "work-id");
        assert_eq!(
            fs::read(p.config_dir.join(".credentials.json")).unwrap(),
            before
        );
    }

    #[test]
    fn system_usage_cache_cannot_validate_the_actual_token_owner() {
        let (tmp, mut p) = fixture();
        p.id = SYSTEM.into();
        p.system = true;
        logged_in(&p, i64::MAX);
        let metadata = json!({
            "oauthAccount": {"accountUuid": "work-id", "emailAddress": "work@example.com"},
            "cachedUsageUtilization": {"accountUuid": "work-id", "utilization": {"five_hour": {"utilization": 2}}}
        });
        write_json(&config_file(&p), &metadata).unwrap();
        let mut index = ProfileIndex {
            accounts: vec![p.clone()],
            ..Default::default()
        };
        save_index(tmp.path(), &index).unwrap();
        assert_eq!(row(&p, false).status, "unverified");
        assert!(switch_in(tmp.path(), SYSTEM).is_err());
        let original_credentials = fs::read(p.config_dir.join(".credentials.json")).unwrap();
        let original_metadata = fs::read(config_file(&p)).unwrap();
        let actual = FakeClient(Ok(
            json!({"account": {"uuid": "another-id", "email": "actual@gmx.es"}}),
        ));
        assert!(verify_with_client(tmp.path(), &mut index, &mut p, &actual).is_err());
        let reloaded = load_index(tmp.path()).unwrap().accounts.remove(0);
        let result = row(&reloaded, false);
        assert_eq!(result.status, "conflict");
        assert!(result.message.contains("ac***@gmx.es"));
        assert!(prepare_system(tmp.path(), reloaded, &BTreeMap::new()).is_err());
        assert_eq!(
            fs::read(p.config_dir.join(".credentials.json")).unwrap(),
            original_credentials
        );
        assert_eq!(fs::read(config_file(&p)).unwrap(), original_metadata);
        let saved = fs::read_to_string(tmp.path().join("profiles.json")).unwrap();
        assert!(!saved.contains("test-access-secret"));
        assert!(!saved.contains("test-refresh-secret"));
    }

    #[test]
    fn system_verification_survives_restart_and_invalidates_when_credentials_change() {
        let (tmp, mut p) = fixture();
        p.id = SYSTEM.into();
        p.system = true;
        logged_in(&p, i64::MAX);
        let mut index = ProfileIndex {
            accounts: vec![p.clone()],
            ..Default::default()
        };
        let actual = FakeClient(Ok(
            json!({"account": {"uuid": "work-id", "email": "work@example.com"}}),
        ));
        verify_with_client(tmp.path(), &mut index, &mut p, &actual).unwrap();
        let mut reloaded = load_index(tmp.path()).unwrap().accounts.remove(0);
        assert_eq!(row(&reloaded, false).status, "ready");
        assert!(
            reloaded.account_uuid.is_empty(),
            "the system entry must follow future legitimate CLI logins"
        );
        let path = reloaded.config_dir.join(".credentials.json");
        let mut root = read_json(&path).unwrap();
        root["claudeAiOauth"]["accessToken"] = json!("new-token-for-an-unknown-account");
        write_json(&path, &root).unwrap();
        assert_eq!(row(&reloaded, false).status, "unverified");
        // A token change also invalidates a previously recorded mismatch.
        reloaded.system_identity.as_mut().unwrap().account_uuid = "another-id".into();
        assert_eq!(row(&reloaded, false).status, "unverified");
    }

    #[test]
    fn legacy_migration_preserves_original_and_requires_fresh_login() {
        let tmp = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4().to_string();
        write_json(&tmp.path().join("index.json"), &json!({"accounts": [{"id": id, "name": "旧工作号", "email": "work@example.com", "accountUuid": "work-id"}]})).unwrap();
        write_json(
            &tmp.path().join(&id).join("credentials.json"),
            &json!({"claudeAiOauth": {"accessToken": "broken-legacy-secret"}}),
        )
        .unwrap();
        let original = fs::read(tmp.path().join(&id).join("credentials.json")).unwrap();
        let legacy = legacy_rows(tmp.path(), &ProfileIndex::default())
            .unwrap()
            .remove(0);
        create_in(
            tmp.path(),
            legacy.name.clone(),
            "inherit".into(),
            Some(legacy),
        )
        .unwrap();
        let p = load_index(tmp.path()).unwrap().accounts.remove(0);
        assert_eq!(p.account_uuid, "work-id");
        assert!(!p.config_dir.join(".credentials.json").exists());
        assert_eq!(
            fs::read(tmp.path().join(&id).join("credentials.json")).unwrap(),
            original
        );
        assert!(legacy_rows(tmp.path(), &load_index(tmp.path()).unwrap())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn removing_keeps_login_and_history_and_resets_default() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        fs::write(p.config_dir.join("history.jsonl"), "history-sentinel").unwrap();
        switch_in(tmp.path(), &p.id).unwrap();
        delete_in(tmp.path(), &p.id).unwrap();
        assert!(p.config_dir.join(".credentials.json").exists());
        assert_eq!(
            fs::read_to_string(p.config_dir.join("history.jsonl")).unwrap(),
            "history-sentinel"
        );
        let index = load_index(tmp.path()).unwrap();
        assert_eq!(selected(&index).unwrap().id, SYSTEM);
        assert!(index.accounts.is_empty());
        assert_eq!(index.retired[0].id, p.id);
    }

    #[test]
    fn resuming_uses_original_owner_after_default_changes_and_removal() {
        let (tmp, p) = fixture();
        let cwd = "D:/project";
        let projects = p
            .config_dir
            .join("projects")
            .join(crate::tools::claude::encode_project_dir(cwd));
        fs::create_dir_all(&projects).unwrap();
        fs::write(projects.join("session-1.jsonl"), "").unwrap();
        assert_eq!(
            owner_in(tmp.path(), cwd, Some("session-1")).unwrap().id,
            p.id
        );
        assert_eq!(owner_in(tmp.path(), cwd, None).unwrap().id, SYSTEM);
        delete_in(tmp.path(), &p.id).unwrap();
        assert_eq!(
            owner_in(tmp.path(), cwd, Some("session-1")).unwrap().id,
            p.id
        );
        assert!(owner_in(tmp.path(), cwd, Some("missing")).is_err());
    }

    #[test]
    fn invalid_ids_cannot_escape_store_and_corrupt_index_is_not_overwritten() {
        for id in ["../escape", "C:\\escape", "..", "", "system/child"] {
            assert!(validate_id(id).is_err());
        }
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("profiles.json"), "invalid-index").unwrap();
        assert!(list_in(tmp.path()).is_err());
        assert!(create_in(tmp.path(), "账号".into(), "inherit".into(), None).is_err());
        assert_eq!(
            fs::read_to_string(tmp.path().join("profiles.json")).unwrap(),
            "invalid-index"
        );
    }

    #[test]
    fn settings_override_detection_reports_keys_without_exposing_values() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        let settings = p.config_dir.join("settings.json");
        write_json(
            &settings,
            &json!({"env": {"ANTHROPIC_API_KEY": "secret-api-value"}}),
        )
        .unwrap();
        let problem = auth_override(&[settings]).unwrap();
        assert!(problem.contains("ANTHROPIC_API_KEY"));
        assert!(!problem.contains("secret-api-value"));
        assert_eq!(row(&p, false).status, "conflict");
        assert!(switch_in(tmp.path(), &p.id).is_err());
    }

    #[test]
    fn clearing_logs_out_only_selected_account_and_retains_history_and_other_credentials() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        let second_id = create_in(tmp.path(), "个人".into(), "off".into(), None).unwrap();
        let second = load_index(tmp.path()).unwrap().accounts.into_iter().find(|a| a.id == second_id).unwrap();
        logged_in(&second, i64::MAX);
        let other = fs::read(second.config_dir.join(".credentials.json")).unwrap();
        fs::write(p.config_dir.join("history.jsonl"), "history").unwrap();
        let legacy = tmp.path().join(&p.id).join("credentials.json");
        write_json(&legacy, &json!({"claudeAiOauth":{"accessToken":"old-secret"},"keep":"other-provider"})).unwrap();
        let stopped = std::cell::Cell::new(false);
        let result = clear_credentials_in(tmp.path(), &p.id, || { stopped.set(true); Ok(()) }, |profile| {
            assert!(stopped.get());
            assert_eq!(profile.id, p.id);
            fs::remove_file(profile.config_dir.join(".credentials.json")).unwrap();
            Ok(())
        }).unwrap();
        assert_eq!(result.accounts.iter().find(|a| a.id == p.id).unwrap().status, "signedOut");
        assert_eq!(fs::read_to_string(p.config_dir.join("history.jsonl")).unwrap(), "history");
        assert_eq!(fs::read(second.config_dir.join(".credentials.json")).unwrap(), other);
        assert_eq!(read_json(&legacy).unwrap(), json!({"keep":"other-provider"}));
    }

    #[test]
    fn clearing_aborts_when_terminal_stop_or_official_logout_fails() {
        let (tmp, p) = fixture();
        logged_in(&p, i64::MAX);
        let before = fs::read(p.config_dir.join(".credentials.json")).unwrap();
        assert!(clear_credentials_in(tmp.path(), &p.id, || Err("still running".into()), |_| panic!("logout must not run")).is_err());
        assert!(clear_credentials_in(tmp.path(), &p.id, || Ok(()), |_| Err("logout failed".into())).is_err());
        // A CLI success response alone is insufficient if credentials remain.
        assert!(clear_credentials_in(tmp.path(), &p.id, || Ok(()), |_| Ok(())).is_err());
        assert_eq!(fs::read(p.config_dir.join(".credentials.json")).unwrap(), before);
    }

    #[test]
    fn legacy_clear_removes_both_saved_tokens_without_running_a_cli() {
        let tmp = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4().to_string();
        write_json(&tmp.path().join("index.json"), &json!({"accounts":[{"id":id,"name":"旧账号"}]})).unwrap();
        for name in ["credentials.json", "pending-credentials.json"] {
            write_json(&tmp.path().join(&id).join(name), &json!({"claudeAiOauth":{"refreshToken":"old-secret"},"keep":true})).unwrap();
        }
        clear_credentials_in(tmp.path(), &id, || panic!("legacy has no running profile"), |_| panic!("legacy must not log out system")).unwrap();
        for name in ["credentials.json", "pending-credentials.json"] {
            assert_eq!(read_json(&tmp.path().join(&id).join(name)).unwrap(), json!({"keep":true}));
        }
    }

    #[test]
    fn repair_replaces_stale_system_identity_only_after_verification_and_preserves_settings() {
        let (tmp, mut p) = fixture();
        p.id = SYSTEM.into(); p.system = true;
        logged_in(&p, i64::MAX);
        let original = json!({"oauthAccount":{"accountUuid":"old-id","emailAddress":"old@example.com","organizationUuid":"old-org"},"projects":{"project":{"trusted":true}},"mcpServers":{"server":{"command":"local"}}});
        write_json(&config_file(&p), &original).unwrap();
        let tokens = fs::read(p.config_dir.join(".credentials.json")).unwrap();
        let mut index = ProfileIndex { accounts: vec![p.clone()], ..Default::default() };
        for response in [Err(AuthError::Network), Ok(json!({"account":{"uuid":"actual-id"}}))] {
            assert!(repair_system_in(tmp.path(), &mut index, &mut p, &FakeClient(response)).is_err());
            assert_eq!(read_json(&config_file(&p)).unwrap(), original);
        }
        repair_system_in(tmp.path(), &mut index, &mut p, &FakeClient(Ok(json!({"account":{"uuid":"actual-id","email":"actual@example.com"}})))).unwrap();
        let updated = read_json(&config_file(&p)).unwrap();
        assert_eq!(updated["oauthAccount"]["accountUuid"], "actual-id");
        assert!(updated["oauthAccount"].get("organizationUuid").is_none());
        assert_eq!(updated["projects"], original["projects"]);
        assert_eq!(updated["mcpServers"], original["mcpServers"]);
        assert_eq!(row(&load_index(tmp.path()).unwrap().accounts[0], false).status, "ready");
        assert_eq!(fs::read(p.config_dir.join(".credentials.json")).unwrap(), tokens);
    }

    #[test]
    fn repair_rejects_token_change_during_verification_without_overwriting_cache() {
        struct ChangingClient(PathBuf);
        impl OAuthClient for ChangingClient {
            fn profile(&self, _: &Value) -> Result<Value, AuthError> {
                write_json(&self.0, &json!({"claudeAiOauth":{"accessToken":"changed-token"}})).unwrap();
                Ok(json!({"account":{"uuid":"old-verified-id","email":"old@example.com"}}))
            }
        }
        let (tmp, mut p) = fixture();
        p.id = SYSTEM.into(); p.system = true;
        logged_in(&p, i64::MAX);
        let before = fs::read(config_file(&p)).unwrap();
        let mut index = ProfileIndex::default();
        let client = ChangingClient(p.config_dir.join(".credentials.json"));
        assert!(repair_system_in(tmp.path(), &mut index, &mut p, &client).is_err());
        assert_eq!(fs::read(config_file(&p)).unwrap(), before);
    }

    #[test]
    fn cross_account_continuation_is_owned_by_target_and_default_is_unchanged() {
        let (tmp, source) = fixture();
        let id = Uuid::new_v4().to_string();
        let cwd = "D:/project";
        let project = source.config_dir.join("projects").join(crate::tools::claude::encode_project_dir(cwd));
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(format!("{id}.jsonl")), format!("{}\n", json!({"type":"user","sessionId":id,"message":{"content":"继续"}}))).unwrap();
        let target_id = create_in(tmp.path(), "个人".into(), "off".into(), None).unwrap();
        assert!(continue_in(tmp.path(), cwd, &id, &target_id, "任务").is_err());
        let target = load_index(tmp.path()).unwrap().accounts.into_iter().find(|a| a.id == target_id).unwrap();
        logged_in(&target, i64::MAX);
        let copied = continue_in(tmp.path(), cwd, &id, &target_id, "任务").unwrap();
        assert_eq!(copied.account_id, target_id);
        assert_eq!(owner_in(tmp.path(), cwd, Some(&copied.session_id)).unwrap().id, target_id);
        assert_eq!(owner_in(tmp.path(), cwd, Some(&id)).unwrap().id, source.id);
        assert_eq!(selected(&load_index(tmp.path()).unwrap()).unwrap().id, SYSTEM);
        let ledger: HashMap<String, i64> = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("copy-ledger.json")).unwrap(),
        )
        .unwrap();
        assert!(ledger.get(&copied.session_id).is_some_and(|ts| *ts > 0));
    }
}
