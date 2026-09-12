use crate::proxy::validate_proxy_url;
use crate::state::AppSettings;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const HOMEPAGE: &str = "https://ccswitch.io";
pub const RELEASES_URL: &str = "https://github.com/farion1231/cc-switch/releases";
const GITHUB_LATEST: &str = "https://api.github.com/repos/farion1231/cc-switch/releases/latest";
const USER_AGENT: &str = "Agent-Dock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub found: bool,
    pub path: Option<String>,
    pub download_dir: String,
    pub install_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Latest {
    pub version: String,
    pub tag: String,
    pub asset_name: String,
    pub size: u64,
    pub url: String,
    pub homepage: String,
    pub releases_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub ok: bool,
    pub path: String,
    pub version: String,
    pub log: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub message: String,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: Option<String>,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

pub fn host_arch() -> &'static str {
    std::env::consts::ARCH
}

pub fn pick_asset<'a>(assets: &'a [ReleaseAsset], os: &str, arch: &str) -> Option<&'a ReleaseAsset> {
    assets
        .iter()
        .filter_map(|asset| asset_score(&asset.name, os, arch).map(|score| (score, asset)))
        .max_by_key(|(score, asset)| (*score, asset.size))
        .map(|(_, asset)| asset)
}

pub fn asset_score(name: &str, os: &str, arch: &str) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    if n.ends_with(".sig") || n == "latest.json" || n.contains("source") {
        return None;
    }
    let want_arm = matches!(arch, "aarch64" | "arm64");
    let is_arm = n.contains("arm64") || n.contains("aarch64");
    match os {
        "windows" => {
            if !n.contains("windows") || want_arm != is_arm {
                return None;
            }
            if n.contains("portable") && n.ends_with(".zip") {
                Some(3)
            } else if n.ends_with(".msi") {
                Some(1)
            } else {
                None
            }
        }
        "macos" => {
            if !(n.contains("macos") || n.contains("darwin")) {
                return None;
            }
            if n.ends_with(".zip") {
                Some(3)
            } else if n.ends_with(".dmg") {
                Some(1)
            } else {
                None
            }
        }
        "linux" => {
            if !n.contains("linux") || want_arm != is_arm {
                return None;
            }
            if n.ends_with(".appimage") {
                Some(3)
            } else if n.ends_with(".tar.gz") {
                Some(2)
            } else if n.ends_with(".deb") {
                Some(1)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn is_launch_target(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if !stem_matches(name) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    if cfg!(windows) {
        path.is_file() && (lower.ends_with(".exe") || lower.ends_with(".lnk"))
    } else if cfg!(target_os = "macos") {
        (path.is_dir() && lower.ends_with(".app")) || path.is_file()
    } else {
        path.is_file()
    }
}

pub fn stem_matches(name: &str) -> bool {
    let stem = name
        .to_ascii_lowercase()
        .trim_end_matches(".appimage")
        .trim_end_matches(".exe")
        .trim_end_matches(".lnk")
        .trim_end_matches(".app")
        .replace([' ', '_'], "-");
    stem == "cc-switch" || stem == "ccswitch" || stem.starts_with("cc-switch")
}

pub fn windows_start_args(path: &Path) -> Vec<String> {
    vec![
        "/c".into(),
        "start".into(),
        String::new(),
        path.to_string_lossy().into_owned(),
    ]
}

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_install_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Programs")
            .join("CC-Switch")
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/Users/Shared"))
            .join("Applications")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local")
            .join("opt")
            .join("cc-switch")
    }
}

pub fn probe(settings: &AppSettings) -> Probe {
    let path = resolve_installed(settings);
    Probe {
        found: path.is_some(),
        path: path.map(|item| item.display().to_string()),
        download_dir: preferred_dir(&settings.ccswitch_download_dir, default_download_dir())
            .display()
            .to_string(),
        install_dir: preferred_dir(
            &settings.ccswitch_install_dir,
            Path::new(settings.ccswitch_path.trim())
                .parent()
                .filter(|_| !settings.ccswitch_path.trim().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(default_install_dir),
        )
        .display()
        .to_string(),
    }
}

pub fn fetch_latest(proxy_url: Option<&str>) -> Result<Latest, String> {
    let release = match request_json::<GhRelease>(GITHUB_LATEST, proxy_url) {
        Ok(value) => value,
        Err(err) if proxy_url.is_some() => request_json::<GhRelease>(GITHUB_LATEST, None).map_err(|_| err)?,
        Err(err) => return Err(err),
    };
    let assets: Vec<ReleaseAsset> = release
        .assets
        .into_iter()
        .map(|asset| ReleaseAsset {
            name: asset.name,
            size: asset.size,
            url: asset.browser_download_url,
        })
        .collect();
    let os = crate::platform::host_platform();
    let asset = pick_asset(&assets, os, host_arch()).ok_or_else(|| {
        format!("GitHub 最新版没有适合当前系统（{os}/{arch}）的 CC Switch 安装包。", arch = host_arch())
    })?;
    if asset_score(&asset.name, os, host_arch()) == Some(1) {
        return Err(format!(
            "最新版只有安装包 {}，无法解压到自定义目录。请到 {} 下载便携版。",
            asset.name, RELEASES_URL
        ));
    }
    let tag = release.tag_name.trim().to_string();
    let version = tag.trim_start_matches('v').to_string();
    Ok(Latest {
        version,
        tag,
        asset_name: asset.name.clone(),
        size: asset.size,
        url: asset.url.clone(),
        homepage: HOMEPAGE.into(),
        releases_url: release.html_url.unwrap_or_else(|| RELEASES_URL.into()),
    })
}

pub fn install(
    settings: &mut AppSettings,
    download_dir: &str,
    install_dir: &str,
    proxy_url: Option<&str>,
    mut on_progress: impl FnMut(Progress),
) -> Result<InstallResult, String> {
    let mut log = String::new();
    let download_dir = require_dir(download_dir, "下载位置")?;
    let install_dir = require_dir(install_dir, "安装位置")?;
    on_progress(progress("fetch", "正在查询 GitHub 最新版", 0, 0));
    let latest = fetch_latest(proxy_url)?;
    push_line(&mut log, &format!("最新版 {} · {}", latest.version, latest.asset_name));
    let archive = download_dir.join(&latest.asset_name);
    on_progress(progress(
        "download",
        &format!("正在下载 {}", latest.asset_name),
        0,
        latest.size,
    ));
    download_file(&latest.url, &archive, latest.size, proxy_url, &mut on_progress)?;
    push_line(&mut log, &format!("已下载 {}", archive.display()));
    on_progress(progress("install", "正在安装到指定目录", latest.size, latest.size));
    let launched = install_archive(&archive, &install_dir, &mut log)?;
    settings.ccswitch_path = launched.display().to_string();
    settings.ccswitch_download_dir = download_dir.display().to_string();
    settings.ccswitch_install_dir = install_dir.display().to_string();
    push_line(&mut log, &format!("可执行文件 {}", launched.display()));
    on_progress(progress("done", "安装完成", latest.size, latest.size));
    Ok(InstallResult {
        ok: true,
        path: launched.display().to_string(),
        version: latest.version,
        log,
    })
}

pub fn remember_path(settings: &mut AppSettings, path: &str) -> Result<String, String> {
    let path = PathBuf::from(path.trim());
    if !is_launch_target(&path) {
        return Err("请选择 CC Switch 的可执行文件或应用包".into());
    }
    let resolved = path.display().to_string();
    settings.ccswitch_path = resolved.clone();
    if let Some(parent) = path.parent() {
        if parent.is_dir() {
            settings.ccswitch_install_dir = parent.display().to_string();
        }
    }
    Ok(resolved)
}

pub fn launch(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    if !is_launch_target(&path) {
        return Err("找不到 CC Switch，请先安装或重新选择程序位置。".into());
    }
    spawn_detached(&path)
}

pub fn find_in(dir: &Path, depth: u32) -> Option<PathBuf> {
    if is_launch_target(dir) {
        return Some(dir.to_path_buf());
    }
    if depth == 0 || !dir.is_dir() {
        return None;
    }
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.eq_ignore_ascii_case(".app") || name.to_ascii_lowercase().ends_with(".app") {
        return None;
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for path in &entries {
        if is_launch_target(path) {
            return Some(path.clone());
        }
    }
    if depth <= 1 {
        return None;
    }
    for path in entries {
        if path.is_dir() {
            if let Some(found) = find_in(&path, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|err| format!("无法创建安装目录：{err}"))?;
    let file = File::open(zip_path).map_err(|err| format!("无法读取安装包：{err}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| format!("安装包不是有效的 zip：{err}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("读取压缩条目失败：{err}"))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("安装包包含不安全路径：{}", entry.name()));
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|err| format!("无法创建目录：{err}"))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("无法创建目录：{err}"))?;
        }
        let mut outfile = File::create(&out).map_err(|err| format!("无法写入 {}：{err}", out.display()))?;
        io::copy(&mut entry, &mut outfile).map_err(|err| format!("解压失败：{err}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

fn resolve_installed(settings: &AppSettings) -> Option<PathBuf> {
    let configured = settings.ccswitch_path.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if is_launch_target(&path) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_in(&path, 3) {
                return Some(found);
            }
        }
    }
    let install = settings.ccswitch_install_dir.trim();
    if !install.is_empty() {
        if let Some(found) = find_in(Path::new(install), 3) {
            return Some(found);
        }
    }
    for dir in candidate_dirs() {
        if let Some(found) = find_in(&dir, 3) {
            return Some(found);
        }
    }
    for name in ["cc-switch", "CC-Switch", "ccswitch", "CC Switch"] {
        if let Some(found) = crate::platform::which_cmd(name) {
            if is_launch_target(&found) {
                return Some(found);
            }
        }
    }
    None
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(default_install_dir());
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("Applications"));
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".local").join("opt").join("cc-switch"));
        #[cfg(windows)]
        {
            dirs.push(home.join("AppData").join("Local").join("Programs").join("CC-Switch"));
            dirs.push(home.join("AppData").join("Local").join("Programs").join("CC Switch"));
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join("Programs").join("CC-Switch"));
            dirs.push(local.join("Programs").join("CC Switch"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(pf.clone()).join("CC-Switch"));
            dirs.push(PathBuf::from(pf).join("CC Switch"));
        }
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(pf86.clone()).join("CC-Switch"));
            dirs.push(PathBuf::from(pf86).join("CC Switch"));
        }
        if let Some(roaming) = dirs::config_dir() {
            dirs.push(
                roaming
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            dirs.push(
                PathBuf::from(program_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        if let Some(desktop) = dirs::desktop_dir() {
            dirs.push(desktop);
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/Applications"));
        dirs.push(PathBuf::from("/Applications/CC Switch.app"));
        dirs.push(PathBuf::from("/Applications/CC-Switch.app"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/opt/cc-switch"));
    }
    dirs
}

fn install_archive(archive: &Path, install_dir: &Path, log: &mut String) -> Result<PathBuf, String> {
    let name = archive
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".zip") {
        extract_zip(archive, install_dir)?;
        push_line(log, &format!("已解压到 {}", install_dir.display()));
    } else if name.ends_with(".appimage") {
        let dest = install_dir.join(archive.file_name().unwrap_or_else(|| std::ffi::OsStr::new("cc-switch.AppImage")));
        fs::copy(archive, &dest).map_err(|err| format!("无法复制 AppImage：{err}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest)
                .map_err(|err| format!("无法读取 AppImage 权限：{err}"))?
                .permissions();
            perms.set_mode(perms.mode() | 0o755);
            fs::set_permissions(&dest, perms).map_err(|err| format!("无法设置可执行权限：{err}"))?;
        }
        push_line(log, &format!("已复制到 {}", dest.display()));
        return Ok(dest);
    } else {
        return Err(format!(
            "暂不支持自动安装 {}。请解压便携版 zip，或从 {} 手动安装。",
            archive.display(),
            RELEASES_URL
        ));
    }
    find_in(install_dir, 4).ok_or_else(|| {
        format!(
            "安装包已解压到 {}，但没有找到 CC Switch 可执行文件。请检查目录。",
            install_dir.display()
        )
    })
}

fn download_file(
    url: &str,
    dest: &Path,
    expected: u64,
    proxy_url: Option<&str>,
    on_progress: &mut impl FnMut(Progress),
) -> Result<(), String> {
    if dest.is_file() {
        if let Ok(meta) = dest.metadata() {
            if expected > 0 && meta.len() == expected {
                on_progress(progress("download", "安装包已在下载目录，跳过下载", expected, expected));
                return Ok(());
            }
        }
    }
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|s| s.to_str()).unwrap_or("bin")
    ));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建下载目录：{err}"))?;
    }
    let response = match http_get(url, proxy_url) {
        Ok(resp) => resp,
        Err(err) if proxy_url.is_some() => http_get(url, None).map_err(|_| err)?,
        Err(err) => return Err(err),
    };
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(expected);
    let mut reader = response.into_reader();
    let mut file = File::create(&tmp).map_err(|err| format!("无法写入下载文件：{err}"))?;
    let mut buf = [0_u8; 64 * 1024];
    let mut received = 0_u64;
    let mut last_emit = 0_u64;
    loop {
        let n = reader.read(&mut buf).map_err(|err| format!("下载中断：{err}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|err| format!("写入下载文件失败：{err}"))?;
        received += n as u64;
        if received.saturating_sub(last_emit) >= 512 * 1024 || received == total {
            on_progress(progress(
                "download",
                &format!("正在下载（{} / {}）", format_bytes(received), format_bytes(total)),
                received,
                total,
            ));
            last_emit = received;
        }
    }
    file.flush().map_err(|err| format!("无法保存下载文件：{err}"))?;
    drop(file);
    if expected > 0 && received != expected && total > 0 && received != total {
        let _ = fs::remove_file(&tmp);
        return Err(format!("下载不完整（{received} / {expected} 字节），请重试。"));
    }
    fs::rename(&tmp, dest).map_err(|err| format!("无法保存安装包：{err}"))?;
    Ok(())
}

fn http_get(url: &str, proxy_url: Option<&str>) -> Result<ureq::Response, String> {
    let agent = http_agent(proxy_url)?;
    match agent.get(url).set("User-Agent", USER_AGENT).set("Accept", "*/*").call() {
        Ok(resp) => Ok(resp),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(http_status_message(code, &body))
        }
        Err(_) => Err(if proxy_url.is_some() {
            "走代理下载失败，请检查代理地址。".into()
        } else {
            format!("无法连接 GitHub。可填写代理后重试，或打开 {RELEASES_URL} 手动下载。")
        }),
    }
}

fn request_json<T: for<'de> Deserialize<'de>>(url: &str, proxy_url: Option<&str>) -> Result<T, String> {
    let agent = http_agent(proxy_url)?;
    let response = match agent
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            return Err(http_status_message(code, &body));
        }
        Err(_) => {
            return Err(if proxy_url.is_some() {
                "走代理查询 GitHub 失败，请检查代理地址。".into()
            } else {
                format!("无法连接 GitHub 获取最新版。可填写代理后重试，或打开 {RELEASES_URL}。")
            });
        }
    };
    let body = response.into_string().map_err(|_| "GitHub 返回无法读取。".to_string())?;
    serde_json::from_str(&body).map_err(|_| "GitHub 最新版信息无法解析。".to_string())
}

fn http_agent(proxy_url: Option<&str>) -> Result<ureq::Agent, String> {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout(Duration::from_secs(600));
    if let Some(url) = proxy_url.map(str::trim).filter(|item| !item.is_empty()) {
        let url = validate_proxy_url(url)?;
        let proxy = ureq::Proxy::new(&url).map_err(|_| "代理地址无法用于下载 CC Switch。".to_string())?;
        builder = builder.proxy(proxy);
    }
    Ok(builder.build())
}

fn http_status_message(code: u16, body: &str) -> String {
    match code {
        403 | 429 => "GitHub 接口限流，请稍后再试，或填写代理。".into(),
        404 => "GitHub 上找不到 CC Switch 最新版。".into(),
        _ => {
            if body.trim().is_empty() {
                format!("GitHub 返回 {code}。")
            } else {
                format!("GitHub 返回 {code}。")
            }
        }
    }
}

fn spawn_detached(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let args = windows_start_args(path);
        Command::new("cmd")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|err| format!("无法打开 CC Switch：{err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let mut cmd = if path.extension().and_then(|s| s.to_str()) == Some("app") || path.is_dir() {
            let mut cmd = Command::new("open");
            cmd.arg(path);
            cmd
        } else {
            Command::new(path)
        };
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("无法启动 CC Switch：{err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::{Command, Stdio};
        Command::new(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("无法启动 CC Switch：{err}"))?;
        return Ok(());
    }
}

fn require_dir(path: &str, label: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(format!("请选择{label}"));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(format!("{label}必须是绝对路径"));
    }
    fs::create_dir_all(&path).map_err(|err| format!("无法创建{label}：{err}"))?;
    if !path.is_dir() {
        return Err(format!("{label}不是文件夹"));
    }
    Ok(path)
}

fn preferred_dir(saved: &str, fallback: PathBuf) -> PathBuf {
    let trimmed = saved.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        PathBuf::from(trimmed)
    }
}

fn progress(stage: &str, message: &str, received: u64, total: u64) -> Progress {
    Progress {
        stage: stage.into(),
        message: message.into(),
        received,
        total,
    }
}

fn push_line(log: &mut String, line: &str) {
    if !log.is_empty() && !log.ends_with('\n') {
        log.push('\n');
    }
    log.push_str(line);
    log.push('\n');
}

pub fn format_bytes(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let n = value as f64;
    if n >= GB {
        format!("{:.1} GB", n / GB)
    } else if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.0} KB", n / KB)
    } else {
        format!("{value} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn asset(name: &str, size: u64) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            size,
            url: format!("https://example.invalid/{name}"),
        }
    }

    fn sample_assets() -> Vec<ReleaseAsset> {
        vec![
            asset("CC-Switch-v3.20.3-Linux-arm64.AppImage", 10),
            asset("CC-Switch-v3.20.3-Linux-x86_64.AppImage", 11),
            asset("CC-Switch-v3.20.3-Linux-x86_64.deb", 4),
            asset("CC-Switch-v3.20.3-macOS.dmg", 20),
            asset("CC-Switch-v3.20.3-macOS.zip", 21),
            asset("CC-Switch-v3.20.3-Windows-arm64-Portable.zip", 12),
            asset("CC-Switch-v3.20.3-Windows-arm64.msi", 8),
            asset("CC-Switch-v3.20.3-Windows-Portable.zip", 13),
            asset("CC-Switch-v3.20.3-Windows.msi", 9),
            asset("CC-Switch-v3.20.3-Windows.msi.sig", 1),
            asset("latest.json", 2),
        ]
    }

    #[test]
    fn windows_x64_prefers_portable_zip_not_arm() {
        let assets = sample_assets();
        let picked = pick_asset(&assets, "windows", "x86_64").unwrap();
        assert_eq!(picked.name, "CC-Switch-v3.20.3-Windows-Portable.zip");
    }

    #[test]
    fn windows_arm_prefers_arm_portable() {
        let assets = sample_assets();
        let picked = pick_asset(&assets, "windows", "aarch64").unwrap();
        assert_eq!(picked.name, "CC-Switch-v3.20.3-Windows-arm64-Portable.zip");
    }

    #[test]
    fn macos_prefers_zip_over_dmg() {
        let assets = sample_assets();
        let picked = pick_asset(&assets, "macos", "aarch64").unwrap();
        assert_eq!(picked.name, "CC-Switch-v3.20.3-macOS.zip");
    }

    #[test]
    fn linux_x64_prefers_appimage() {
        let assets = sample_assets();
        let picked = pick_asset(&assets, "linux", "x86_64").unwrap();
        assert_eq!(picked.name, "CC-Switch-v3.20.3-Linux-x86_64.AppImage");
    }

    #[test]
    fn linux_arm_skips_x64_appimage() {
        let assets = sample_assets();
        let picked = pick_asset(&assets, "linux", "aarch64").unwrap();
        assert_eq!(picked.name, "CC-Switch-v3.20.3-Linux-arm64.AppImage");
    }

    #[test]
    fn stem_matches_common_names() {
        assert!(stem_matches("CC-Switch.exe"));
        assert!(stem_matches("CC Switch.app"));
        assert!(stem_matches("CC Switch.lnk"));
        assert!(stem_matches("cc-switch.AppImage"));
        assert!(stem_matches("ccswitch"));
        assert!(!stem_matches("switch.exe"));
        assert!(!stem_matches("latest.json"));
    }

    #[test]
    fn windows_start_keeps_path_with_spaces_as_one_arg() {
        let path = PathBuf::from(r"C:\Users\PS\AppData\Local\Programs\CC Switch\cc-switch.exe");
        let args = windows_start_args(&path);
        assert_eq!(args[0], "/c");
        assert_eq!(args[1], "start");
        assert_eq!(args[2], "");
        assert_eq!(args[3], path.to_string_lossy());
    }

    #[test]
    fn extract_zip_writes_nested_exe_and_rejects_slip() {
        let root = tempfile::tempdir().unwrap();
        let zip_path = root.path().join("app.zip");
        let dest = root.path().join("dest");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("CC-Switch/CC-Switch.exe", options).unwrap();
            zip.write_all(b"mz").unwrap();
            zip.finish().unwrap();
        }
        extract_zip(&zip_path, &dest).unwrap();
        let found = find_in(&dest, 4).unwrap();
        assert_eq!(found.file_name().unwrap().to_string_lossy(), "CC-Switch.exe");

        let slip = root.path().join("slip.zip");
        {
            let file = File::create(&slip).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("../evil.exe", options).unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        let err = extract_zip(&slip, &dest).unwrap_err();
        assert!(err.contains("不安全路径"), "{err}");
    }

    #[test]
    fn format_bytes_uses_mb_for_release_assets() {
        assert_eq!(format_bytes(13_658_645), "13.0 MB");
        assert_eq!(format_bytes(800), "800 B");
    }
}
