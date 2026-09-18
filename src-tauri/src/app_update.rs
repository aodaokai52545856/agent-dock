use crate::tools::update::{
    app_version, compare_label, http_download_file, http_get_text, parse_version, proxy_pairs,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const GITHUB_REPO: &str = "aodaokai52545856/agent-dock";
const GITHUB_LATEST: &str = "https://api.github.com/repos/aodaokai52545856/agent-dock/releases/latest";
const GITHUB_RELEASES: &str = "https://api.github.com/repos/aodaokai52545856/agent-dock/releases?per_page=20";
const MIN_ASSET_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    WinSetup,
    WinPortable,
    MacDmg,
    MacApp,
    LinuxAppImage,
    LinuxDeb,
}

impl ClientKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WinSetup => "win-setup",
            Self::WinPortable => "win-exe",
            Self::MacDmg => "mac-dmg",
            Self::MacApp => "mac-app",
            Self::LinuxAppImage => "linux-appimage",
            Self::LinuxDeb => "linux-deb",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::WinSetup => "Windows 安装包",
            Self::WinPortable => "Windows 便携版",
            Self::MacDmg => "macOS 磁盘映像",
            Self::MacApp => "macOS 应用包",
            Self::LinuxAppImage => "Linux AppImage",
            Self::LinuxDeb => "Linux deb",
        }
    }

    fn asset_name(self, version: &str) -> String {
        match self {
            Self::WinSetup => format!("AgentDock-{version}-Setup.exe"),
            Self::WinPortable => format!("AgentDock-{version}.exe"),
            Self::MacDmg => format!("AgentDock-{version}.dmg"),
            Self::MacApp => format!("AgentDock-{version}.app.tar.gz"),
            Self::LinuxAppImage => format!("AgentDock-{version}.AppImage"),
            Self::LinuxDeb => format!("AgentDock-{version}.deb"),
        }
    }

    fn fallback(self) -> Option<Self> {
        match self {
            Self::WinSetup => Some(Self::WinPortable),
            Self::WinPortable => Some(Self::WinSetup),
            Self::MacDmg => Some(Self::MacApp),
            Self::MacApp => Some(Self::MacDmg),
            Self::LinuxDeb => Some(Self::LinuxAppImage),
            Self::LinuxAppImage => Some(Self::LinuxDeb),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub local_version: String,
    pub latest_version: String,
    pub compare: String,
    pub kind: String,
    pub kind_label: String,
    pub asset_name: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpgradeResult {
    pub ok: bool,
    pub log: String,
    pub restart: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

pub fn detect_client_kind(exe: &Path) -> ClientKind {
    detect_client_kind_for(std::env::consts::OS, exe)
}

pub fn detect_client_kind_for(os: &str, exe: &Path) -> ClientKind {
    let name = exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let path = exe.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    match os {
        "windows" => {
            if name.starts_with("agentdock-") && name.ends_with(".exe") && !name.contains("setup") {
                ClientKind::WinPortable
            } else if path.contains("/agent dock/")
                || path.contains("/com.aitools.agentdock/")
                || name == "agent dock.exe"
                || exe
                    .parent()
                    .map(|dir| dir.join("uninstall.exe").is_file())
                    .unwrap_or(false)
            {
                ClientKind::WinSetup
            } else if name.ends_with(".exe") && !name.contains("setup") && name.contains("agentdock") {
                ClientKind::WinPortable
            } else {
                ClientKind::WinSetup
            }
        }
        "macos" => {
            if path.contains(".app.tar.gz") {
                ClientKind::MacApp
            } else {
                ClientKind::MacDmg
            }
        }
        _ => {
            if std::env::var_os("APPIMAGE").is_some() || name.ends_with(".appimage") {
                ClientKind::LinuxAppImage
            } else if path.starts_with("/usr/") || name.ends_with(".deb") {
                ClientKind::LinuxDeb
            } else {
                ClientKind::LinuxAppImage
            }
        }
    }
}

fn pick_asset<'a>(
    assets: &'a [GhAsset],
    kind: ClientKind,
    version: &str,
) -> Option<(ClientKind, &'a GhAsset)> {
    let mut current = Some(kind);
    while let Some(next) = current {
        let want = next.asset_name(version).to_ascii_lowercase();
        if let Some(asset) = assets.iter().find(|item| item.name.to_ascii_lowercase() == want) {
            return Some((next, asset));
        }
        current = next.fallback().filter(|other| *other != kind);
    }
    None
}

pub fn check_app_update(proxy_url: Option<&str>) -> Result<AppUpdateInfo, String> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("Agent Dock.exe"));
    check_app_update_with(&exe, proxy_url)
}

fn check_app_update_with(exe: &Path, proxy_url: Option<&str>) -> Result<AppUpdateInfo, String> {
    let local = app_version();
    let kind = detect_client_kind(exe);
    let proxy = proxy_pairs(proxy_url.unwrap_or(""));
    let release = fetch_latest_release(&proxy)?;
    let latest = parse_version(&release.tag_name).unwrap_or_else(|| release.tag_name.trim_start_matches('v').to_string());
    let (kind, asset) = pick_asset(&release.assets, kind, &latest).ok_or_else(|| {
        format!(
            "最新版本 {latest} 没有当前客户端（{}）对应的安装包",
            kind.label()
        )
    })?;
    Ok(AppUpdateInfo {
        local_version: local.clone(),
        latest_version: latest.clone(),
        compare: compare_label(&local, &latest),
        kind: kind.as_str().into(),
        kind_label: kind.label().into(),
        asset_name: asset.name.clone(),
        html_url: release.html_url,
    })
}

pub fn upgrade_app(proxy_url: Option<&str>) -> Result<AppUpgradeResult, String> {
    let exe = std::env::current_exe().map_err(|err| format!("无法定位当前程序：{err}"))?;
    upgrade_app_with(&exe, proxy_url)
}

fn upgrade_app_with(exe: &Path, proxy_url: Option<&str>) -> Result<AppUpgradeResult, String> {
    let mut log = String::new();
    let info = check_app_update_with(exe, proxy_url)?;
    push_line(&mut log, &format!("当前 {} · 最新 {}", info.local_version, info.latest_version));
    push_line(&mut log, &format!("安装包类型：{}", info.kind_label));
    if info.compare != "可更新" {
        push_line(&mut log, "已经是最新版本，无需下载。");
        return Ok(AppUpgradeResult {
            ok: true,
            log,
            restart: false,
        });
    }
    let proxy = proxy_pairs(proxy_url.unwrap_or(""));
    let release = fetch_latest_release(&proxy)?;
    let latest = parse_version(&release.tag_name).unwrap_or_else(|| release.tag_name.trim_start_matches('v').to_string());
    let kind = detect_client_kind(exe);
    let (kind, asset) = pick_asset(&release.assets, kind, &latest).ok_or_else(|| {
        format!("最新版本没有 {} 安装包", kind.label())
    })?;
    if asset.size > 0 && asset.size < MIN_ASSET_BYTES {
        return Err(format!("安装包异常小（{} 字节）", asset.size));
    }
    let dest = download_dir()?.join(&asset.name);
    push_line(&mut log, &format!("$ GET {}", asset.browser_download_url));
    push_line(&mut log, "正在下载与当前客户端同类的安装包…");
    let size = http_download_file(&asset.browser_download_url, &proxy, &dest)?;
    if size < MIN_ASSET_BYTES {
        let _ = fs::remove_file(&dest);
        return Err(format!("下载文件过小（{size} 字节），可能不是安装包"));
    }
    push_line(&mut log, &format!("已下载 {size} 字节 → {}", dest.display()));
    let restart = apply_package(kind, &dest, exe, &mut log)?;
    Ok(AppUpgradeResult {
        ok: true,
        log,
        restart,
    })
}

fn fetch_latest_release(proxy: &[(String, String)]) -> Result<GhRelease, String> {
    match http_get_text(GITHUB_LATEST, proxy) {
        Ok(text) => parse_release_json(&text),
        Err(err) if is_http_not_found(&err) => fetch_newest_from_list(proxy),
        Err(err) => fetch_newest_from_list(proxy).map_err(|list_err| {
            format!("读取 GitHub 版本失败：{err}；列表接口：{list_err}（{GITHUB_REPO}）")
        }),
    }
}

fn fetch_newest_from_list(proxy: &[(String, String)]) -> Result<GhRelease, String> {
    let text = http_get_text(GITHUB_RELEASES, proxy)
        .map_err(|err| format!("读取 GitHub 版本列表失败：{err}（{GITHUB_REPO}）"))?;
    let releases: Vec<GhRelease> =
        serde_json::from_str(&text).map_err(|err| format!("无法解析 GitHub 版本列表：{err}"))?;
    pick_newest_release(releases)
}

fn parse_release_json(text: &str) -> Result<GhRelease, String> {
    serde_json::from_str(text).map_err(|err| format!("无法解析 GitHub 版本信息：{err}"))
}

fn pick_newest_release(releases: Vec<GhRelease>) -> Result<GhRelease, String> {
    let usable: Vec<GhRelease> = releases.into_iter().filter(|item| !item.draft).collect();
    if usable.is_empty() {
        return Err("GitHub 上还没有可用的发布版本".into());
    }
    if let Some(stable) = usable.iter().find(|item| !item.prerelease) {
        return Ok(stable.clone());
    }
    Ok(usable.into_iter().next().expect("usable is not empty"))
}

fn is_http_not_found(err: &str) -> bool {
    err.contains("404") || err.to_ascii_lowercase().contains("not found")
}

fn download_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("agent-dock-update");
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建下载目录：{err}"))?;
    Ok(dir)
}

fn apply_package(kind: ClientKind, package: &Path, current_exe: &Path, log: &mut String) -> Result<bool, String> {
    match kind {
        ClientKind::WinSetup => {
            push_line(log, "启动安装程序，完成后会替换当前客户端。");
            spawn_detached(package, &["/S"])?;
            Ok(true)
        }
        ClientKind::WinPortable | ClientKind::LinuxAppImage => {
            replace_running_exe(current_exe, package, log)?;
            Ok(true)
        }
        ClientKind::MacDmg | ClientKind::MacApp => {
            push_line(log, "已打开安装包，请按系统提示替换 Agent Dock。");
            open_path(package)?;
            Ok(false)
        }
        ClientKind::LinuxDeb => {
            push_line(log, "已打开 deb 安装包，请用系统安装器完成更新。");
            open_path(package)?;
            Ok(false)
        }
    }
}

fn replace_running_exe(current: &Path, downloaded: &Path, log: &mut String) -> Result<(), String> {
    let dest = current.to_path_buf();
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "agent-dock".into());
    let bak = dest.with_file_name(format!("{name}.bak"));
    if bak.exists() {
        let _ = fs::remove_file(&bak);
    }
    if dest.exists() {
        fs::rename(&dest, &bak).map_err(|err| format!("无法备份当前程序：{err}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(downloaded)
            .map_err(|err| format!("无法读取下载文件：{err}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(downloaded, perms).map_err(|err| format!("无法设置可执行权限：{err}"))?;
    }
    if fs::rename(downloaded, &dest).is_err() {
        fs::copy(downloaded, &dest).map_err(|err| format!("无法替换当前程序：{err}"))?;
        let _ = fs::remove_file(downloaded);
    }
    push_line(log, &format!("已替换 {}", dest.display()));
    spawn_detached(&dest, &[])?;
    Ok(())
}

fn spawn_detached(path: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(path);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED: u32 = 0x00000008 | 0x00000200;
        cmd.creation_flags(DETACHED);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("无法启动安装程序：{err}"))
}

fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("无法打开安装包：{err}"))
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("无法打开安装包：{err}"))
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("无法打开安装包：{err}"))
    }
}

fn push_line(log: &mut String, line: &str) {
    log.push_str(line);
    if !line.ends_with('\n') {
        log.push('\n');
    }
}

pub fn schedule_exit(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GhAsset {
        GhAsset {
            name: name.into(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 4_000_000,
        }
    }

    #[test]
    fn portable_exe_is_not_the_nsis_setup() {
        let kind = detect_client_kind_for(
            "windows",
            Path::new(r"D:\Downloads\AgentDock-0.1.2.exe"),
        );
        assert_eq!(kind, ClientKind::WinPortable);
        assert_eq!(kind.asset_name("0.1.3"), "AgentDock-0.1.3.exe");
    }

    #[test]
    fn installed_windows_client_uses_setup() {
        let kind = detect_client_kind_for(
            "windows",
            Path::new(r"C:\Users\me\AppData\Local\Agent Dock\Agent Dock.exe"),
        );
        assert_eq!(kind, ClientKind::WinSetup);
        assert_eq!(kind.asset_name("0.1.3"), "AgentDock-0.1.3-Setup.exe");
    }

    #[test]
    fn linux_appimage_and_deb_follow_the_running_file() {
        assert_eq!(
            detect_client_kind_for("linux", Path::new("/home/me/AgentDock-0.1.2.AppImage")),
            ClientKind::LinuxAppImage
        );
        assert_eq!(
            detect_client_kind_for("linux", Path::new("/usr/bin/agent-dock")),
            ClientKind::LinuxDeb
        );
        assert_eq!(
            detect_client_kind_for("macos", Path::new("/Applications/Agent Dock.app/Contents/MacOS/agent-dock")),
            ClientKind::MacDmg
        );
    }

    #[test]
    fn pick_asset_falls_back_to_the_sibling_package() {
        let assets = vec![
            asset("AgentDock-0.1.3.exe"),
            asset("AgentDock-0.1.3.AppImage"),
        ];
        let (kind, hit) = pick_asset(&assets, ClientKind::WinSetup, "0.1.3").unwrap();
        assert_eq!(kind, ClientKind::WinPortable);
        assert_eq!(hit.name, "AgentDock-0.1.3.exe");
    }

    #[test]
    fn github_release_json_yields_tag_and_assets() {
        let raw = r#"{
          "tag_name":"v0.1.3",
          "html_url":"https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.3",
          "assets":[{"name":"AgentDock-0.1.3-Setup.exe","browser_download_url":"https://example.invalid/setup","size":3500000}]
        }"#;
        let release: GhRelease = serde_json::from_str(raw).unwrap();
        assert_eq!(parse_version(&release.tag_name).as_deref(), Some("0.1.3"));
        let (kind, asset) = pick_asset(&release.assets, ClientKind::WinSetup, "0.1.3").unwrap();
        assert_eq!(kind, ClientKind::WinSetup);
        assert!(asset.browser_download_url.contains("setup"));
    }

    #[test]
    fn newest_list_skips_drafts_and_prefers_stable() {
        let releases = vec![
            GhRelease {
                tag_name: "v0.1.3".into(),
                html_url: "https://example.invalid/v0.1.3".into(),
                draft: false,
                prerelease: true,
                assets: vec![asset("AgentDock-0.1.3-Setup.exe")],
            },
            GhRelease {
                tag_name: "v0.1.2".into(),
                html_url: "https://example.invalid/v0.1.2".into(),
                draft: false,
                prerelease: false,
                assets: vec![asset("AgentDock-0.1.2-Setup.exe")],
            },
        ];
        let picked = pick_newest_release(releases).unwrap();
        assert_eq!(picked.tag_name, "v0.1.2");
    }

    #[test]
    fn newest_list_uses_prerelease_when_that_is_all() {
        let releases = vec![GhRelease {
            tag_name: "v0.1.2".into(),
            html_url: "https://example.invalid/v0.1.2".into(),
            draft: false,
            prerelease: true,
            assets: vec![asset("AgentDock-0.1.2-Setup.exe")],
        }];
        let picked = pick_newest_release(releases).unwrap();
        assert_eq!(picked.tag_name, "v0.1.2");
        assert!(picked.prerelease);
        assert!(is_http_not_found("下载失败：https://api.github.com/...: status code 404"));
    }
}
