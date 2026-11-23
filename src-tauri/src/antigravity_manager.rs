use crate::platform;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

const SWITCH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerState {
    pub accounts: Vec<ManagerAccount>,
    pub current_account_id: Option<String>,
    pub data_dir: String,
    pub manager_path: Option<String>,
    pub can_switch: bool,
    pub switch_hint: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerAccount {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub subscription: Option<String>,
    pub disabled: bool,
    pub quotas: Vec<ManagerQuota>,
    pub quota_updated_at: Option<i64>,
    pub last_used: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerQuota {
    pub name: String,
    pub percentage: u8,
    pub reset_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
}

// This projection of the upstream storage format keeps credential fields out of Dock DTOs.
#[derive(Default, Deserialize)]
struct AccountIndex {
    accounts: Vec<IndexedAccount>,
    current_agy_account_id: Option<String>,
    current_account_id: Option<String>,
    current_target_ide: Option<String>,
}

#[derive(Deserialize)]
struct IndexedAccount {
    id: String,
    email: String,
    name: Option<String>,
    #[serde(default)]
    disabled: bool,
    last_used: Option<i64>,
}

#[derive(Default, Deserialize)]
struct AccountDetails {
    quota: Option<QuotaSnapshot>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    banned: bool,
}

#[derive(Deserialize)]
struct QuotaSnapshot {
    subscription_tier: Option<String>,
    last_updated: Option<i64>,
    quota_groups: Option<Vec<QuotaGroup>>,
    #[serde(default)]
    models: Vec<ModelQuota>,
}

#[derive(Deserialize)]
struct QuotaGroup {
    display_name: String,
    #[serde(default)]
    buckets: Vec<QuotaBucket>,
}

#[derive(Deserialize)]
struct QuotaBucket {
    bucket_id: Option<String>,
    display_name: Option<String>,
    window: String,
    remaining_fraction: f64,
    reset_time: Option<String>,
}

#[derive(Deserialize)]
struct ModelQuota {
    name: String,
    display_name: Option<String>,
    percentage: i32,
    reset_time: Option<String>,
}

pub(crate) fn data_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("无法找到用户目录")?;
    if let Some(path) = configured_path(std::env::var("ABV_DATA_DIR").ok(), &home) {
        return Ok(path);
    }
    let pointer = configured_path(std::env::var("ABV_DATA_DIR_POINTER_FILE").ok(), &home)
        .unwrap_or_else(|| home.join(".antigravity_tools_location"));
    let path = configured_path(fs::read_to_string(pointer).ok(), &home);
    Ok(path.unwrap_or_else(|| home.join(".antigravity_tools")))
}

fn configured_path(value: Option<String>, home: &Path) -> Option<PathBuf> {
    let value = value
        .as_deref()?
        .trim()
        .trim_matches(['\u{feff}', '"', '\'']);
    if value.is_empty() {
        return None;
    }
    if value == "~" {
        return Some(home.to_path_buf());
    }
    if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return Some(home.join(rest));
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Some(path);
    }
    std::env::current_dir().ok().map(|cwd| cwd.join(path))
}

fn find_manager() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let install_roots = dirs::data_local_dir()
            .into_iter()
            .chain(dirs::home_dir().map(|home| home.join("AppData/Local")))
            .chain(std::env::var_os("ProgramFiles").map(PathBuf::from))
            .chain(std::env::var_os("ProgramW6432").map(PathBuf::from))
            .chain(std::env::var_os("ProgramFiles(x86)").map(PathBuf::from));
        return install_roots
            .flat_map(|root| [root.clone(), root.join("Programs")])
            .map(|dir| dir.join("Antigravity Tools").join("antigravity-tools.exe"))
            .find(|path| path.is_file())
            .or_else(|| platform::which_cmd("antigravity-tools.exe"));
    }
    #[cfg(target_os = "macos")]
    {
        let executable = "Antigravity Tools.app/Contents/MacOS/antigravity-tools";
        return dirs::home_dir()
            .map(|home| home.join("Applications").join(executable))
            .into_iter()
            .chain([Path::new("/Applications").join(executable)])
            .find(|path| path.is_file())
            .or_else(|| platform::which_cmd("antigravity-tools"));
    }
    #[cfg(target_os = "linux")]
    {
        crate::antigravity_tools_install::linux_appimage_path()
            .filter(|path| path.is_file())
            .or_else(|| platform::which_cmd("antigravity-tools"))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        platform::which_cmd("antigravity-tools")
    }
}

fn read_json<T: DeserializeOwned>(file: File) -> Result<T, String> {
    let mut reader = BufReader::new(file);
    if reader
        .fill_buf()
        .is_ok_and(|bytes| bytes.starts_with(b"\xef\xbb\xbf"))
    {
        reader.consume(3);
    }
    serde_json::from_reader(reader).map_err(|_| "Antigravity Tools 的账号数据无法解析".into())
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn to_millis(timestamp: Option<i64>) -> Option<i64> {
    timestamp
        .filter(|timestamp| *timestamp > 0)?
        .checked_mul(1000)
}

impl QuotaSnapshot {
    fn display_quotas(self) -> Vec<ManagerQuota> {
        let group_quotas = self
            .quota_groups
            .unwrap_or_default()
            .into_iter()
            .flat_map(|group| {
                // Derive the group identifier from the bucket ID prefix used by Antigravity Tools.
                let group_id = group.buckets.iter().find_map(|bucket| {
                    let bucket_id = bucket.bucket_id.as_deref()?.trim();
                    let window_suffix = format!("-{}", bucket.window);
                    let group_id = bucket_id.strip_suffix(&window_suffix).unwrap_or(bucket_id);
                    (!group_id.is_empty()).then(|| group_id.to_owned())
                });
                let group_name = nonempty(Some(group.display_name.clone()));
                group.buckets.into_iter().map(move |bucket| ManagerQuota {
                    name: nonempty(bucket.display_name)
                        .unwrap_or_else(|| format!("{} · {}", group.display_name, bucket.window)),
                    percentage: (bucket.remaining_fraction * 100.0)
                        .round()
                        .clamp(0.0, 100.0) as u8,
                    reset_time: nonempty(bucket.reset_time),
                    group_id: group_id.clone(),
                    group_name: group_name.clone(),
                    window: nonempty(Some(bucket.window)),
                })
            })
            .collect::<Vec<_>>();
        if !group_quotas.is_empty() {
            return group_quotas;
        }
        self.models
            .into_iter()
            .map(|model| ManagerQuota {
                name: nonempty(model.display_name).unwrap_or(model.name),
                percentage: model.percentage.clamp(0, 100) as u8,
                reset_time: nonempty(model.reset_time),
                group_id: None,
                group_name: None,
                window: None,
            })
            .collect()
    }
}

fn list_in(root: &Path, manager: Option<&Path>) -> Result<ManagerState, String> {
    let index: AccountIndex = match File::open(root.join("accounts.json")) {
        Ok(file) => read_json(file)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => AccountIndex::default(),
        Err(err) => return Err(format!("无法读取 Antigravity Tools 的账号索引：{err}")),
    };
    let current_id = index.current_agy_account_id.or_else(|| {
        (index.current_target_ide.as_deref() == Some("agy"))
            .then_some(index.current_account_id)
            .flatten()
    });
    let mut seen = HashSet::new();
    let accounts = index
        .accounts
        .into_iter()
        .filter(|account| {
            if account.email.trim().is_empty() {
                return false;
            }
            Uuid::parse_str(&account.id).is_ok_and(|id| {
                id.hyphenated()
                    .to_string()
                    .eq_ignore_ascii_case(&account.id)
                    && seen.insert(id)
            })
        })
        .map(|account| {
            let profile = root.join("accounts").join(format!("{}.json", account.id));
            let details: AccountDetails = File::open(profile)
                .ok()
                .and_then(|file| read_json(file).ok())
                .unwrap_or_default();
            let (subscription, quota_updated_at, quotas) =
                details.quota.map_or((None, None, Vec::new()), |mut quota| {
                    (
                        nonempty(quota.subscription_tier.take()),
                        to_millis(quota.last_updated),
                        quota.display_quotas(),
                    )
                });
            ManagerAccount {
                id: account.id,
                email: account.email,
                name: nonempty(account.name),
                subscription,
                disabled: account.disabled || details.disabled || details.banned,
                quotas,
                quota_updated_at,
                last_used: to_millis(account.last_used),
            }
        })
        .collect::<Vec<_>>();
    let current_account_id = current_id.filter(|id| accounts.iter().any(|row| &row.id == id));
    Ok(ManagerState {
        accounts,
        current_account_id,
        data_dir: root.to_string_lossy().into_owned(),
        manager_path: manager.map(|path| path.to_string_lossy().into_owned()),
        can_switch: manager.is_some(),
        switch_hint: manager
            .is_none()
            .then(|| "请安装 Antigravity Tools 后再切换账号".into()),
    })
}

pub fn list() -> Result<ManagerState, String> {
    list_in(&data_dir()?, find_manager().as_deref())
}

pub fn switch(account_id: &str) -> Result<ManagerState, String> {
    let root = data_dir()?;
    let manager = find_manager().ok_or("未找到 Antigravity Tools，请先安装")?;
    let state = list_in(&root, Some(&manager))?;
    let account = state
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .ok_or("Antigravity Tools 中找不到此账号，请刷新列表")?;
    if account.disabled {
        return Err("该账号已停用，请在 Antigravity Tools 中处理".into());
    }

    // The native switcher can contact the manager's loopback API before its offline fallback.
    let no_proxy = std::env::var("no_proxy")
        .or_else(|_| std::env::var("NO_PROXY"))
        .map_or_else(
            |_| "127.0.0.1".to_string(),
            |existing| format!("127.0.0.1,{existing}"),
        );
    let mut command = Command::new(&manager);
    command
        // This targets the native agy login via the manager's own account switcher.
        .args(["switch", account_id, "--target", "agy"])
        .env("ABV_DATA_DIR", &root)
        .env("NO_PROXY", &no_proxy)
        .env("no_proxy", &no_proxy)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    platform::prepare_command(&mut command);
    platform::apply_no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| format!("无法运行 Antigravity Tools：{err}"))?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("无法检查 Antigravity Tools 切换结果：{err}"))?
        {
            break status;
        }
        if start.elapsed() >= SWITCH_TIMEOUT {
            if child.kill().is_ok() {
                let _ = child.wait();
            }
            return Err("等待切换超时，请在 Antigravity Tools 中确认当前 agy 账号及版本".into());
        }
        std::thread::sleep(Duration::from_millis(75));
    };
    if !status.success() {
        return Err("切换失败，请在 Antigravity Tools 检查账号和 agy 登录状态".into());
    }

    let state = list_in(&root, Some(&manager))?;
    if state.current_account_id.as_deref() != Some(account_id) {
        return Err("Antigravity Tools 未确认 agy 账号切换，请检查版本或在其界面完成切换".into());
    }
    Ok(state)
}

pub fn launch() -> Result<(), String> {
    let manager = find_manager().ok_or("未找到 Antigravity Tools，请先安装")?;
    let mut command = Command::new(manager);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    platform::prepare_command(&mut command);
    platform::apply_no_window(&mut command);
    let child = command
        .spawn()
        .map_err(|err| format!("无法打开 Antigravity Tools：{err}"))?;
    #[cfg(unix)]
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    #[cfg(not(unix))]
    drop(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AGY_ID: &str = "12345678-1111-4111-8111-123456789abc";
    const IDE_ID: &str = "87654321-1111-4111-8111-123456789abc";

    #[test]
    fn reads_antigravity_tools_metadata_without_returning_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = dir.path().join("accounts");
        fs::create_dir(&accounts).unwrap();
        fs::write(
            dir.path().join("accounts.json"),
            json!({
                "current_account_id": IDE_ID,
                "current_target_ide": "ide",
                "current_agy_account_id": AGY_ID,
                "accounts": [
                    {"id": AGY_ID, "email": "cli@example.test", "name": "CLI", "last_used": 1_700_000_000},
                    {"id": IDE_ID, "email": "ide@example.test"},
                    {"id": "../other", "email": "outside@example.test"}
                ]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            accounts.join(format!("{AGY_ID}.json")),
            json!({
                "token": {"refresh_token": "credential-marker"},
                "quota": {
                    "subscription_tier": "PRO",
                    "last_updated": 1_700_000_000,
                    "quota_groups": [{
                        "display_name": "GEMINI",
                        "buckets": [
                            {"bucket_id": "gemini-5h", "window": "5h", "display_name": "Five Hour Limit Remaining", "remaining_fraction": 0.871},
                            {"window": "weekly", "remaining_fraction": 0.12, "reset_time": "2026-11-30T00:00:00Z"}
                        ]
                    }, {
                        "display_name": "Claude and GPT models",
                        "buckets": [
                            {"bucket_id": "3p-5h", "window": "5h", "display_name": "Five Hour Limit Remaining", "remaining_fraction": 0.7}
                        ]
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            accounts.join(format!("{IDE_ID}.json")),
            json!({
                "disabled": true,
                "quota": { "models": [{"name": "gemini", "percentage": 120}] }
            })
            .to_string(),
        )
        .unwrap();

        let state = list_in(dir.path(), None).unwrap();
        assert!(!state.can_switch);
        assert_eq!(state.current_account_id.as_deref(), Some(AGY_ID));
        assert_eq!(state.accounts.len(), 2);
        assert_eq!(state.accounts[0].quota_updated_at, Some(1_700_000_000_000));
        assert_eq!(state.accounts[0].quotas[0].percentage, 87);
        assert_eq!(
            state.accounts[0].quotas[0].group_name.as_deref(),
            Some("GEMINI")
        );
        assert_eq!(state.accounts[0].quotas[0].window.as_deref(), Some("5h"));
        assert_eq!(
            state.accounts[0].quotas[0].group_id.as_deref(),
            Some("gemini")
        );
        assert_eq!(state.accounts[0].quotas[1].name, "GEMINI · weekly");
        assert_eq!(
            state.accounts[0].quotas[1].group_id.as_deref(),
            Some("gemini")
        );
        assert_eq!(
            state.accounts[0].quotas[2].name,
            state.accounts[0].quotas[0].name
        );
        assert_eq!(state.accounts[0].quotas[2].group_id.as_deref(), Some("3p"));
        assert!(state.accounts[1].disabled);
        assert_eq!(state.accounts[1].quotas[0].percentage, 100);
        assert!(state.accounts[1].quotas[0].group_id.is_none());
        assert!(state.accounts[1].quotas[0].group_name.is_none());
        assert!(state.accounts[1].quotas[0].window.is_none());
        assert!(!serde_json::to_string(&state)
            .unwrap()
            .contains("credential-marker"));
    }

    #[test]
    fn never_marks_an_ide_only_account_as_current_for_agy() {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("accounts.json");
        let value = |target| {
            json!({
                "current_account_id": IDE_ID,
                "current_target_ide": target,
                "accounts": [{"id": IDE_ID, "email": "account@example.test"}]
            })
            .to_string()
        };
        fs::write(&index, value("classic")).unwrap();
        assert!(list_in(dir.path(), None)
            .unwrap()
            .current_account_id
            .is_none());
        fs::write(index, value("agy")).unwrap();
        assert_eq!(
            list_in(dir.path(), None)
                .unwrap()
                .current_account_id
                .as_deref(),
            Some(IDE_ID)
        );
    }
}
