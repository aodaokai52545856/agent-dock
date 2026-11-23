use crate::ccswitch::{self, Latest, Progress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const HOMEPAGE: &str = "https://github.com/lbjlaq/Antigravity-Manager";
pub const RELEASES_URL: &str = "https://github.com/lbjlaq/Antigravity-Manager/releases";
const GITHUB_LATEST: &str =
    "https://api.github.com/repos/lbjlaq/Antigravity-Manager/releases/latest";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub path: String,
    pub version: String,
    pub log: String,
    pub launched: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

struct Package {
    latest: Latest,
    sha256: String,
}

pub fn fetch_latest(proxy_url: Option<&str>) -> Result<Latest, String> {
    fetch_package(proxy_url).map(|package| package.latest)
}

pub fn install(
    proxy_url: Option<&str>,
    mut on_progress: impl FnMut(Progress),
) -> Result<InstallResult, String> {
    on_progress(ccswitch::progress(
        "fetch",
        "正在查询 Antigravity Tools 最新版",
        0,
        0,
    ));
    let package = fetch_package(proxy_url)?;
    let latest = &package.latest;
    let path = install_path(&latest.asset_name)?;
    on_progress(ccswitch::progress(
        "download",
        &format!("正在下载 {}", latest.asset_name),
        0,
        latest.size,
    ));
    download_package(&package, &path, proxy_url, &mut on_progress)?;
    let (launched, message) = open_installer(&path)?;
    on_progress(ccswitch::progress(
        "done",
        message,
        latest.size,
        latest.size,
    ));
    Ok(InstallResult {
        path: path.display().to_string(),
        version: latest.version.clone(),
        log: message.to_string(),
        launched,
    })
}

fn fetch_package(proxy_url: Option<&str>) -> Result<Package, String> {
    let release: Release = github_get(GITHUB_LATEST, "application/vnd.github+json", proxy_url)?
        .into_json()
        .map_err(|_| "无法读取 Antigravity Tools 发布信息。".to_string())?;
    select_package(
        release,
        crate::platform::host_platform(),
        ccswitch::host_arch(),
    )
}

fn select_package(release: Release, os: &str, arch: &str) -> Result<Package, String> {
    let tag = release.tag_name;
    let version = tag.strip_prefix('v').unwrap_or(&tag);
    if !version
        .split('.')
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("Antigravity Tools 的发布版本号格式无效。".into());
    }

    let asset = installer_names(version, os, arch)
        .into_iter()
        .find_map(|name| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.eq_ignore_ascii_case(&name))
        })
        .ok_or_else(|| {
            format!(
                "Antigravity Tools 暂无适用于当前系统（{os}/{arch}）的安装包，请查看官方发布页。"
            )
        })?;
    let url = format!("{RELEASES_URL}/download/{tag}/{}", asset.name);
    if asset.browser_download_url != url || asset.size == 0 {
        return Err("Antigravity Tools 安装包的官方下载信息无效。".into());
    }
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| {
            "Antigravity Tools 安装包缺少有效的 SHA-256 校验值，请查看官方发布页。".to_string()
        })?;

    Ok(Package {
        latest: Latest {
            version: version.to_string(),
            tag: tag.clone(),
            asset_name: asset.name.clone(),
            size: asset.size,
            url,
            homepage: HOMEPAGE.into(),
            releases_url: RELEASES_URL.into(),
        },
        sha256: sha256.to_ascii_lowercase(),
    })
}

fn installer_names(version: &str, os: &str, arch: &str) -> Vec<String> {
    let suffixes: &[&str] = match (os, arch) {
        ("windows", "x86_64") => &["x64-setup.exe", "x64_en-US.msi"],
        ("windows", "aarch64") => &["arm64-setup.exe", "arm64_en-US.msi"],
        ("macos", "x86_64") => &["x64.dmg", "universal.dmg"],
        ("macos", "aarch64") => &["aarch64.dmg", "universal.dmg"],
        ("linux", "x86_64") => &["amd64.AppImage"],
        ("linux", "aarch64") => &["aarch64.AppImage"],
        _ => &[],
    };
    suffixes
        .iter()
        .map(|suffix| format!("Antigravity.Tools_{version}_{suffix}"))
        .collect()
}

fn github_get(url: &str, accept: &str, proxy_url: Option<&str>) -> Result<ureq::Response, String> {
    let get = |proxy| match ccswitch::http_agent(proxy)?
        .get(url)
        .set("User-Agent", "Agent-Dock")
        .set("Accept", accept)
        .call()
    {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(403 | 429, _)) => Err("GitHub 接口暂时受限，请稍后重试。".into()),
        Err(ureq::Error::Status(code, _)) => Err(format!(
            "读取 Antigravity Tools 发布文件失败（GitHub {code}）。"
        )),
        Err(ureq::Error::Transport(_)) => Err("无法连接 GitHub，请检查网络或代理后重试。".into()),
    };
    match get(proxy_url) {
        Err(err) if proxy_url.is_some() => get(None).map_err(|_| err),
        result => result,
    }
}

#[cfg(not(target_os = "linux"))]
fn install_path(asset_name: &str) -> Result<PathBuf, String> {
    let dir = ccswitch::default_download_dir();
    if !dir.is_absolute() {
        return Err("找不到当前用户的下载目录。".into());
    }
    Ok(dir.join("Antigravity Tools").join(asset_name))
}

#[cfg(target_os = "linux")]
fn install_path(_asset_name: &str) -> Result<PathBuf, String> {
    linux_appimage_path().ok_or_else(|| "找不到当前用户的应用数据目录。".into())
}

#[cfg(target_os = "linux")]
pub fn linux_appimage_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| {
        dir.join("agent-dock")
            .join("antigravity-tools")
            .join("antigravity-tools.AppImage")
    })
}

fn download_package(
    package: &Package,
    path: &Path,
    proxy_url: Option<&str>,
    on_progress: &mut impl FnMut(Progress),
) -> Result<(), String> {
    let latest = &package.latest;
    let dir = path
        .parent()
        .ok_or_else(|| "找不到应用安装目录。".to_string())?;
    fs::create_dir_all(dir).map_err(|err| format!("无法创建目录：{err}"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".antigravity-tools-")
        .suffix(".part")
        .tempfile_in(dir)
        .map_err(|err| format!("无法创建临时下载文件：{err}"))?;
    let mut reader = github_get(&latest.url, "application/octet-stream", proxy_url)?.into_reader();
    copy_verified(&mut reader, temporary.as_file_mut(), package, on_progress)?;

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("无法设置 AppImage 的运行权限：{err}"))?;
    }
    temporary
        .persist(path)
        .map_err(|err| format!("无法保存 Antigravity Tools：{}", err.error))?;
    Ok(())
}

fn copy_verified(
    reader: &mut impl Read,
    writer: &mut impl Write,
    package: &Package,
    on_progress: &mut impl FnMut(Progress),
) -> Result<(), String> {
    let total = package.latest.size;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut received = 0;
    let mut last_emit = 0;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|err| format!("下载中断：{err}"))?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > total {
            return Err("安装包大小与官方发布记录不符，请重试。".into());
        }
        let bytes = &buffer[..count];
        writer
            .write_all(bytes)
            .map_err(|err| format!("无法写入下载文件：{err}"))?;
        hasher.update(bytes);
        if received - last_emit >= 512 * 1024 || received == total {
            on_progress(ccswitch::progress(
                "download",
                &format!(
                    "正在下载（{} / {}）",
                    ccswitch::format_bytes(received),
                    ccswitch::format_bytes(total)
                ),
                received,
                total,
            ));
            last_emit = received;
        }
    }
    if received != total {
        return Err("安装包下载不完整，请重试。".into());
    }
    if format!("{:x}", hasher.finalize()) != package.sha256 {
        return Err("安装包的 SHA-256 校验失败，请重试。".into());
    }
    writer
        .flush()
        .map_err(|err| format!("无法保存下载文件：{err}"))
}

#[cfg(windows)]
fn open_installer(path: &Path) -> Result<(bool, &'static str), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let filename: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let open: Vec<u16> = "open".encode_utf16().chain([0]).collect();
    let initialized = unsafe {
        CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    } >= 0;
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            open.as_ptr(),
            filename.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if initialized {
        unsafe { CoUninitialize() };
    }
    if result as isize <= 32 {
        return Err(format!("无法打开安装程序。请手动打开 {}。", path.display()));
    }
    Ok((true, "已打开安装程序，请按提示完成安装。"))
}

#[cfg(target_os = "macos")]
fn open_installer(path: &Path) -> Result<(bool, &'static str), String> {
    let opened = std::process::Command::new("/usr/bin/open")
        .arg(path)
        .status()
        .is_ok_and(|status| status.success());
    if !opened {
        return Err(format!("无法打开磁盘映像。请手动打开 {}。", path.display()));
    }
    Ok((
        true,
        "已打开磁盘映像，请将 Antigravity Tools 拖入“应用程序”。",
    ))
}

#[cfg(target_os = "linux")]
fn open_installer(_path: &Path) -> Result<(bool, &'static str), String> {
    Ok((
        false,
        "已将 AppImage 安装到当前用户目录，可点击打开本机应用。",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            size: 3,
            browser_download_url: format!("{RELEASES_URL}/download/v5.0.4/{name}"),
            digest: Some(format!("sha256:{:x}", Sha256::digest(b"abc"))),
        }
    }

    fn example_release(names: &[&str]) -> Release {
        Release {
            tag_name: "v5.0.4".into(),
            assets: names.iter().map(|name| example_asset(name)).collect(),
        }
    }

    #[test]
    fn selects_native_installers_from_the_upstream_504_assets() {
        let names = [
            "Antigravity.Tools_5.0.4_x64_en-US.msi",
            "Antigravity.Tools_5.0.4_aarch64.AppImage",
            "Antigravity.Tools_5.0.4_amd64.AppImage",
            "Antigravity.Tools_5.0.4_x64-setup.exe.sig",
            "Antigravity.Tools_5.0.4_x64-setup.exe",
            "Antigravity.Tools_5.0.4_aarch64.dmg",
            "Antigravity.Tools_5.0.4_x64.dmg",
            "Antigravity.Tools_5.0.4_arm64.deb",
        ];
        for (os, arch, index) in [
            ("windows", "x86_64", 4),
            ("macos", "x86_64", 6),
            ("macos", "aarch64", 5),
            ("linux", "x86_64", 2),
            ("linux", "aarch64", 1),
        ] {
            let package = select_package(example_release(&names), os, arch).unwrap();
            assert_eq!(package.latest.asset_name, names[index]);
        }
        assert!(select_package(example_release(&names), "windows", "aarch64").is_err());
        let fallback = select_package(example_release(&names[..4]), "windows", "x86_64").unwrap();
        assert_eq!(fallback.latest.asset_name, names[0]);
    }

    #[test]
    fn rejects_incomplete_or_untrusted_download_metadata() {
        let names = ["Antigravity.Tools_5.0.4_x64-setup.exe"];
        let mut release = example_release(&names);
        release.assets[0].browser_download_url = "https://example.invalid/install.exe".into();
        assert!(select_package(release, "windows", "x86_64").is_err());
        let mut release = example_release(&names);
        release.assets[0].digest = None;
        assert!(select_package(release, "windows", "x86_64").is_err());
        let mut release = example_release(&names);
        release.tag_name = "v5.0.4/../preview".into();
        assert!(select_package(release, "windows", "x86_64").is_err());
    }

    #[test]
    fn package_copy_checks_size_and_digest() {
        let package = select_package(
            example_release(&["Antigravity.Tools_5.0.4_x64-setup.exe"]),
            "windows",
            "x86_64",
        )
        .unwrap();
        let mut output = Vec::new();
        copy_verified(&mut &b"abc"[..], &mut output, &package, &mut |_| {}).unwrap();
        assert_eq!(output, b"abc");
        for bytes in [&b"ab"[..], &b"abcx"[..], &b"xyz"[..]] {
            assert!(copy_verified(&mut &*bytes, &mut Vec::new(), &package, &mut |_| {}).is_err());
        }
    }
}
