//! Official OMP binaries and installation-aware updates; no shell scripts.
use super::{resolve_binary, update::UpgradeResult, ToolId};
use crate::{platform, state::AppSettings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Prepared {
    pub version: String,
    pub destination: PathBuf,
    pub download: Option<PathBuf>,
    pub source: String,
    pub sha256: Option<String>,
}
pub fn managed_binary() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_default()
        .join("agent-dock")
        .join("tools")
        .join("omp")
        .join(if cfg!(windows) { "omp.exe" } else { "omp" })
}
fn pending_path() -> PathBuf {
    managed_binary().with_file_name("pending.json")
}
pub fn pending() -> Option<Prepared> {
    serde_json::from_slice(&fs::read(pending_path()).ok()?).ok()
}
pub fn clear_pending() {
    let _ = fs::remove_file(pending_path());
}
pub fn remember(p: &Prepared) -> Result<(), String> {
    fs::create_dir_all(pending_path().parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(
        pending_path(),
        serde_json::to_vec(p).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn source(exe: &Path) -> String {
    let full = exe
        .canonicalize()
        .unwrap_or_else(|_| exe.into())
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    if exe == managed_binary() {
        "dock"
    } else if full.contains("/cellar/") || full.contains("/homebrew/") {
        "homebrew"
    } else if full.contains("/.bun/")
        && exe.parent().and_then(|p| p.parent()).is_some_and(|p| {
            p.join("install/global/node_modules/@oh-my-pi/pi-coding-agent/package.json")
                .is_file()
        })
    {
        "bun"
    } else if full.contains("/node_modules/") || exe.extension().is_some_and(|e| e == "cmd") {
        "npm"
    } else if exe
        .file_name()
        .is_some_and(|n| n == "omp.exe" || n == "omp")
    {
        "native"
    } else {
        "custom"
    }
    .into()
}
fn agent(proxy: Option<&str>) -> Result<ureq::Agent, String> {
    let mut b = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(300));
    if let Some(p) = proxy.filter(|p| !p.trim().is_empty()) {
        b = b.proxy(ureq::Proxy::new(p).map_err(|e| e.to_string())?);
    }
    Ok(b.build())
}
fn target() -> Result<String, String> {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_env = "musl") {
        "linux-musl"
    } else {
        "linux"
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => return Err("OMP 没有此架构的官方二进制".into()),
    };
    Ok(format!("{os}-{arch}"))
}
fn release(proxy: Option<&str>) -> Result<Value, String> {
    agent(proxy)?
        .get(&format!(
            "https://build.stencil.so/api/products/omp/latest/{}?channel=stable",
            target()?
        ))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())
}
pub fn latest(proxy: Option<&str>) -> Result<String, String> {
    release(proxy)?["build"]["version"]
        .as_str()
        .map(str::to_string)
        .ok_or("OMP 版本响应缺少版本号".into())
}
pub fn local_version(exe: &Path) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    platform::apply_no_window(&mut cmd);
    let out = cmd.arg("--version").output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.trim()
        .strip_prefix("omp/")
        .or_else(|| text.trim().strip_prefix("omp v"))
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or(format!("OMP 版本格式异常：{text}"))
}
pub fn prepare(
    settings: &AppSettings,
    proxy: Option<&str>,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Prepared, String> {
    let destination = match resolve_binary(ToolId::Omp, settings) {
        Ok(p) => p,
        Err(e) if !settings.omp_path.trim().is_empty() => return Err(e),
        Err(_) => managed_binary(),
    };
    let source = source(&destination);
    let meta = release(proxy)?;
    let version = meta["build"]["version"]
        .as_str()
        .ok_or("OMP 版本响应无效")?
        .to_string();
    if source != "dock" {
        let p = Prepared {
            version,
            destination,
            download: None,
            source,
            sha256: None,
        };
        remember(&p)?;
        return Ok(p);
    }
    let sha = meta["file"]["sha256"]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("OMP 下载信息缺少 SHA-256")?;
    let url = meta["download"]
        .as_str()
        .filter(|s| s.starts_with("https://"))
        .ok_or("OMP 下载地址无效")?;
    fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
    let download = destination.with_file_name("omp.next");
    let response = agent(proxy)?.get(url).call().map_err(|e| e.to_string())?;
    let total = response
        .header("content-length")
        .and_then(|s| s.parse::<u64>().ok());
    let mut reader = response.into_reader();
    let mut file = fs::File::create(&download).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hash.update(&buf[..n]);
        bytes += n as u64;
        progress(bytes, total);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if format!("{:x}", hash.finalize()) != sha.to_lowercase() || total.is_some_and(|t| t != bytes) {
        return Err("OMP 下载不完整或 SHA-256 校验失败；原版本已保留".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&download, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    if local_version(&download)? != version {
        return Err("下载的 OMP 版本与发布信息不一致".into());
    }
    let p = Prepared {
        version,
        destination,
        download: Some(download),
        source,
        sha256: Some(sha.to_lowercase()),
    };
    remember(&p)?;
    Ok(p)
}
fn install_binary(p: &Prepared, expected: PathBuf) -> Result<(), String> {
    let download = p.download.as_ref().ok_or("OMP 下载尚未完成")?;

    if p.destination != expected || download != &expected.with_file_name("omp.next") {
        return Err("OMP 待应用路径不属于托管安装".into());
    }
    let bytes = fs::read(download).map_err(|e| e.to_string())?;
    if Some(format!("{:x}", Sha256::digest(&bytes))) != p.sha256 {
        return Err("OMP 安装文件校验失败，请重新下载".into());
    }
    let backup = p.destination.with_file_name("omp.previous");
    if p.destination.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        fs::rename(&p.destination, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(download, &p.destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, &p.destination);
        }
        return Err(e.to_string());
    }
    if let Err(e) = local_version(&p.destination).and_then(|v| {
        if v == p.version {
            Ok(v)
        } else {
            Err("OMP 更新后版本不匹配".into())
        }
    }) {
        let _ = fs::remove_file(&p.destination);
        if backup.exists() {
            let _ = fs::rename(&backup, &p.destination);
        }
        return Err(e);
    }

    Ok(())
}
pub fn apply(p: &Prepared, proxy: Option<&str>) -> Result<String, String> {
    if p.download.is_some() {
        install_binary(p, managed_binary())?;
    } else {
        let (exe, args): (PathBuf, Vec<&str>) = match p.source.as_str() {
            "bun" => (
                platform::which_cmd("bun").ok_or("未找到此安装来源的 Bun")?,
                vec!["install", "-g", "@oh-my-pi/pi-coding-agent@latest"],
            ),
            "homebrew" => (
                platform::which_cmd("brew").ok_or("未找到 Homebrew")?,
                vec!["upgrade", "can1357/tap/omp"],
            ),
            "npm" => (
                platform::which_cmd("npm").ok_or("未找到此安装来源的 npm")?,
                vec!["install", "-g", "@oh-my-pi/pi-coding-agent@latest"],
            ),
            "native" => (p.destination.clone(), vec!["update"]),
            _ => return Err("无法安全管理此自定义 OMP 安装，请检查路径与安装来源".into()),
        };
        let mut cmd = Command::new(exe);
        platform::apply_no_window(&mut cmd);
        cmd.args(args);
        if let Some(proxy) = proxy.filter(|p| !p.is_empty()) {
            cmd.envs(crate::proxy::proxy_env(true, proxy)?)
                .env("PI_PROXY", proxy);
        }
        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    let v = local_version(&p.destination)?;
    if v != p.version {
        return Err(format!(
            "执行路径 {} 仍是 {v}，目标版本为 {}。请检查多份 OMP 安装。",
            p.destination.display(),
            p.version
        ));
    }
    clear_pending();
    Ok(format!(
        "OMP 已升级至 {v}\n执行路径：{}",
        p.destination.display()
    ))
}
pub fn upgrade(settings: &AppSettings, proxy: Option<&str>) -> UpgradeResult {
    if !crate::omp_rpc::global().list().is_empty() {
        return UpgradeResult {
            ok: false,
            log: "请在版本中心下载更新并应用，当前 OMP 会话仍在运行".into(),
            local_version: String::new(),
        };
    }
    match prepare(settings, proxy, &mut |_, _| {})
        .and_then(|p| apply(&p, proxy).map(|log| (p.version, log)))
    {
        Ok((local_version, log)) => UpgradeResult {
            ok: true,
            log,
            local_version,
        },
        Err(log) => UpgradeResult {
            ok: false,
            log,
            local_version: String::new(),
        },
    }
}
pub fn uninstall(settings: &AppSettings) -> UpgradeResult {
    let result = (|| -> Result<(), String> {
        if !crate::omp_rpc::global().list().is_empty() {
            return Err("请先关闭 OMP 会话再卸载".into());
        }
        let exe = resolve_binary(ToolId::Omp, settings)?;
        let source = source(&exe);
        if source == "dock" {
            fs::remove_file(exe).map_err(|e| e.to_string())?;
        } else {
            let (manager, args): (&str, Vec<&str>) = match source.as_str() {
                "bun" => ("bun", vec!["remove", "-g", "@oh-my-pi/pi-coding-agent"]),
                "npm" => ("npm", vec!["uninstall", "-g", "@oh-my-pi/pi-coding-agent"]),
                "homebrew" => ("brew", vec!["uninstall", "omp"]),
                _ => return Err("此外部 OMP 独立程序未由 Dock 安装，未删除程序或配置".into()),
            };
            let manager = platform::which_cmd(manager).ok_or("未找到此 OMP 安装来源的包管理器")?;
            let mut cmd = Command::new(manager);
            platform::apply_no_window(&mut cmd);
            let result = cmd.args(args).output().map_err(|e| e.to_string())?;
            if !result.status.success() {
                return Err(String::from_utf8_lossy(&result.stderr).into());
            }
            if exe.exists() {
                return Err("卸载后原执行路径仍存在，请检查多份安装".into());
            }
        }
        clear_pending();
        Ok(())
    })();
    UpgradeResult {
        ok: result.is_ok(),
        log: result
            .err()
            .unwrap_or("已卸载 OMP，配置与历史已保留".into()),
        local_version: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(hash_matches: bool) -> (tempfile::TempDir, Prepared) {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir
            .path()
            .join(if cfg!(windows) { "omp.exe" } else { "omp" });
        let download = destination.with_file_name("omp.next");
        fs::write(&destination, b"previous working program").unwrap();
        fs::write(&download, b"not an executable").unwrap();
        let sha = if hash_matches {
            format!("{:x}", Sha256::digest(b"not an executable"))
        } else {
            "0".repeat(64)
        };
        (
            dir,
            Prepared {
                version: "18.16.0".into(),
                destination,
                download: Some(download),
                source: "dock".into(),
                sha256: Some(sha),
            },
        )
    }
    #[test]
    fn a_bad_checksum_never_replaces_the_existing_program() {
        let (_dir, p) = fixture(false);
        let error = install_binary(&p, p.destination.clone()).unwrap_err();
        assert!(error.contains("校验"));
        assert_eq!(
            fs::read(&p.destination).unwrap(),
            b"previous working program"
        );
        assert!(p.download.as_ref().unwrap().exists());
    }
    #[test]
    fn a_program_that_cannot_start_is_rolled_back() {
        let (_dir, p) = fixture(true);
        assert!(install_binary(&p, p.destination.clone()).is_err());
        assert_eq!(
            fs::read(&p.destination).unwrap(),
            b"previous working program"
        );
    }
    #[test]
    fn a_pending_manifest_cannot_replace_an_unmanaged_path() {
        let (dir, p) = fixture(true);
        assert!(install_binary(&p, dir.path().join("different.exe")).is_err());
        assert_eq!(
            fs::read(&p.destination).unwrap(),
            b"previous working program"
        );
    }
    #[test]
    fn unknown_custom_executables_are_not_treated_as_omp() {
        assert_eq!(source(Path::new("arbitrary.exe")), "custom");
    }
}
