use super::{ToolId, dsh, resolve_binary};
use crate::platform;
use crate::proxy::proxy_env;
use crate::state::AppSettings;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const KIMI_DOWNLOAD_BASE: &str = "https://code.kimi.com/kimi-code";
const KIMI_LATEST_URL: &str = "https://code.kimi.com/kimi-code/latest";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const HTTP_TEXT_READ_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_BIN_READ_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersionInfo {
    pub tool_id: ToolId,
    pub name: String,
    pub found: bool,
    pub path: Option<String>,
    pub local_version: String,
    pub latest_version: String,
    pub compare: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<ToolCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeResult {
    pub ok: bool,
    pub log: String,
    pub local_version: String,
}

pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
pub fn official_url(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Opencode => "https://opencode.ai",
        ToolId::Grokbuild => "https://github.com/xai-org/grok-build#installation",
        ToolId::Kimi => "https://code.kimi.com",
        ToolId::Claude => "https://code.claude.com/docs/en/quickstart",
        ToolId::Pi => "https://pi.dev",
        ToolId::Dsh => "https://github.com/deepseek-ai/deepseek-harness",
    }
}

pub fn version_tools(tool: Option<ToolId>) -> Vec<ToolId> {
    match tool {
        Some(id) => vec![id],
        None => vec![
            ToolId::Opencode,
            ToolId::Grokbuild,
            ToolId::Kimi,
            ToolId::Claude,
            ToolId::Pi,
            ToolId::Dsh,
        ],
    }
}

pub fn list_versions(
    settings: &AppSettings,
    proxy_url: Option<&str>,
    tool: Option<ToolId>,
) -> Vec<ToolVersionInfo> {
    version_tools(tool)
        .into_iter()
        .map(|item| version_info(item, settings, proxy_url))
        .collect()
}

pub fn upgrade_tool(tool: ToolId, settings: &AppSettings, proxy_url: Option<&str>) -> UpgradeResult {
    let mut log = String::new();
    let proxy = proxy_pairs(proxy_url.unwrap_or(&settings.default_proxy_url));
    let result = match tool {
        ToolId::Opencode => upgrade_via_npm(&mut log, "opencode-ai", &proxy),
        ToolId::Kimi => upgrade_kimi(&mut log, settings, &proxy),
        ToolId::Grokbuild => upgrade_grok(&mut log, settings, &proxy),
        ToolId::Claude => upgrade_claude(&mut log, settings, &proxy),
        ToolId::Pi => upgrade_via_npm_flags(
            &mut log,
            "@earendil-works/pi-coding-agent",
            &["--ignore-scripts"],
            &proxy,
        ),
        ToolId::Dsh => upgrade_via_npm(&mut log, "@deepseek-ai/dsh", &proxy),
    };
    if tool == ToolId::Dsh {
        let report = dsh::verify_install(settings);
        if !log.ends_with('\n') && !log.is_empty() {
            log.push('\n');
        }
        log.push_str(&report.log());
        if !log.ends_with('\n') {
            log.push('\n');
        }
        return finish_result(tool, settings, result && report.ok(), log);
    }
    finish_result(tool, settings, result, log)
}

pub fn uninstall_tool(tool: ToolId, settings: &AppSettings) -> UpgradeResult {
    let mut log = String::new();
    let mut did = false;
    if tool == ToolId::Kimi {
        did |= uninstall_kimi_official(&mut log);
    }
    let mut seen = Vec::new();
    loop {
        let path = match resolve_binary(tool, settings) {
            Ok(path) => path,
            Err(_) => break,
        };
        let key = normalize_path_text(&path);
        if seen.iter().any(|item| item == &key) {
            push_line(
                &mut log,
                &format!("仍能找到 {}，停止以免循环。", path.display()),
            );
            break;
        }
        seen.push(key);
        did |= uninstall_one(&path, tool, &mut log);
    }
    let gone = resolve_binary(tool, settings).is_err();
    if !did && seen.is_empty() && gone {
        return UpgradeResult {
            ok: false,
            log: format!("没有找到 {} 命令行。", tool.display_name()),
            local_version: "未安装".into(),
        };
    }
    if gone {
        push_line(&mut log, &format!("{} 命令行已卸载，会话和配置未改动。", tool.display_name()));
    }
    finish_result(tool, settings, did && gone, log)
}

fn finish_result(tool: ToolId, settings: &AppSettings, ok: bool, log: String) -> UpgradeResult {
    let local = match resolve_binary(tool, settings) {
        Ok(path) => local_version(tool, &path),
        Err(_) => "未安装".into(),
    };
    UpgradeResult {
        ok,
        log,
        local_version: local,
    }
}

fn version_info(tool: ToolId, settings: &AppSettings, proxy_url: Option<&str>) -> ToolVersionInfo {
    if tool == ToolId::Dsh {
        return dsh_version_info(settings, proxy_url);
    }
    let resolved = resolve_binary(tool, settings).ok();
    let local = resolved
        .as_ref()
        .map(|path| local_version(tool, path))
        .unwrap_or_else(|| "未安装".into());
    let latest = remote_version(tool, settings, resolved.as_deref(), proxy_url);
    let compare = compare_label(&local, &latest);
    ToolVersionInfo {
        tool_id: tool,
        name: tool.display_name().to_string(),
        found: resolved.is_some(),
        path: resolved.map(|path| path.display().to_string()),
        local_version: local,
        latest_version: latest,
        compare,
        checks: Vec::new(),
    }
}

fn dsh_version_info(settings: &AppSettings, proxy_url: Option<&str>) -> ToolVersionInfo {
    let report = dsh::verify_install(settings);
    let resolved = resolve_binary(ToolId::Dsh, settings).ok();
    let local = if report.ok() {
        resolved
            .as_ref()
            .map(|path| local_version(ToolId::Dsh, path))
            .unwrap_or_else(|| "未知".into())
    } else if resolved.is_none() {
        "未安装".into()
    } else {
        "未通过检查".into()
    };
    let latest = remote_version(ToolId::Dsh, settings, resolved.as_deref(), proxy_url);
    let compare = if report.ok() {
        compare_label(&local, &latest)
    } else if resolved.is_none() {
        "未安装".into()
    } else {
        "未通过检查".into()
    };
    ToolVersionInfo {
        tool_id: ToolId::Dsh,
        name: ToolId::Dsh.display_name().to_string(),
        found: report.ok(),
        path: report.path.or_else(|| resolved.map(|path| path.display().to_string())),
        local_version: local,
        latest_version: latest,
        compare,
        checks: report
            .checks
            .into_iter()
            .map(|item| ToolCheck {
                name: item.name,
                ok: item.ok,
                detail: item.detail,
            })
            .collect(),
    }
}

fn version_args(tool: ToolId) -> &'static [&'static str] {
    match tool {
        ToolId::Grokbuild => &["version"],
        ToolId::Dsh => &["-V"],
        ToolId::Opencode | ToolId::Kimi | ToolId::Claude | ToolId::Pi => &["--version"],
    }
}

fn local_version(tool: ToolId, exe: &Path) -> String {
    let cli = run_capture(exe, version_args(tool), &[]);
    if let Ok(text) = &cli {
        if let Some(ver) = parse_version(text) {
            return ver;
        }
    }
    if let Some(ver) = version_from_npm_layout(exe, npm_package(tool)) {
        return ver;
    }
    match cli {
        Ok(_) => "未知".into(),
        Err(_) => "异常".into(),
    }
}

fn version_from_npm_layout(exe: &Path, pkg: &str) -> Option<String> {
    if pkg.is_empty() || pkg == "grok" {
        return None;
    }
    let mut dir = exe.parent()?;
    for _ in 0..8 {
        let nested = dir.join("node_modules").join(pkg).join("package.json");
        if let Some(ver) = read_npm_package_version(&nested) {
            return Some(ver);
        }
        let direct = dir.join("package.json");
        if let Some(ver) = read_npm_package_version_named(&direct, pkg) {
            return Some(ver);
        }
        dir = dir.parent()?;
    }
    None
}

fn read_npm_package_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let ver = value.get("version")?.as_str()?.trim();
    if ver.is_empty() {
        return None;
    }
    Some(ver.to_string())
}

fn read_npm_package_version_named(path: &Path, pkg: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("name")?.as_str()? != pkg {
        return None;
    }
    let ver = value.get("version")?.as_str()?.trim();
    if ver.is_empty() {
        return None;
    }
    Some(ver.to_string())
}

fn remote_version(tool: ToolId, settings: &AppSettings, exe: Option<&Path>, proxy_url: Option<&str>) -> String {
    let proxy = proxy_pairs(proxy_url.unwrap_or(&settings.default_proxy_url));
    match tool {
        ToolId::Opencode => npm_view("opencode-ai", &proxy),
        ToolId::Kimi => match exe {
            Some(path) if is_native_kimi(path) => kimi_remote_latest(&proxy)
                .or_else(|_| npm_view_result("@moonshot-ai/kimi-code", &proxy))
                .unwrap_or_else(|_| "查询失败".into()),
            _ => npm_view("@moonshot-ai/kimi-code", &proxy),
        },
        ToolId::Claude => npm_view("@anthropic-ai/claude-code", &proxy),
        ToolId::Pi => npm_view("@earendil-works/pi-coding-agent", &proxy),
        ToolId::Dsh => npm_view("@deepseek-ai/dsh", &proxy),
        ToolId::Grokbuild => match exe {
            Some(path) => match run_capture(path, &["update", "--check"], &proxy) {
                Ok(text) => parse_remote_from_check(&text).unwrap_or_else(|| "查询失败".into()),
                Err(_) => "查询失败".into(),
            },
            None => "未安装".into(),
        },
    }
}

fn npm_view(pkg: &str, proxy: &[(String, String)]) -> String {
    npm_view_result(pkg, proxy).unwrap_or_else(|_| "查询失败".into())
}

fn npm_view_result(pkg: &str, proxy: &[(String, String)]) -> Result<String, String> {
    match run_npm(&["view", pkg, "version"], proxy) {
        Ok(text) => parse_version(&text).ok_or_else(|| "查询失败".into()),
        Err(_) if !proxy.is_empty() => match run_npm(&["view", pkg, "version"], &[]) {
            Ok(text) => parse_version(&text).ok_or_else(|| "查询失败".into()),
            Err(_) => Err("查询失败".into()),
        },
        Err(_) => Err("查询失败".into()),
    }
}

fn upgrade_via_npm(log: &mut String, pkg: &str, proxy: &[(String, String)]) -> bool {
    upgrade_via_npm_flags(log, pkg, &[], proxy)
}

fn upgrade_via_npm_flags(
    log: &mut String,
    pkg: &str,
    extra: &[&str],
    proxy: &[(String, String)],
) -> bool {
    if find_npm().is_none() {
        log.push_str("未找到 npm，无法升级。请先安装 Node.js / npm。\n");
        return false;
    }
    let spec = format!("{pkg}@latest");
    let mut args = vec!["install", "-g"];
    args.extend_from_slice(extra);
    args.push(&spec);
    push_line(log, &format!("$ npm {}", args.join(" ")));
    match run_npm(&args, proxy) {
        Ok(text) => {
            log.push_str(&text);
            if !text.ends_with('\n') {
                log.push('\n');
            }
            true
        }
        Err(err) => {
            push_line(log, &err);
            false
        }
    }
}

fn upgrade_kimi(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
    let resolved = resolve_binary(ToolId::Kimi, settings).ok();
    match kimi_upgrade_plan(resolved.as_deref()) {
        KimiUpgradePlan::NativeOnly | KimiUpgradePlan::NativeThenNpm => {
            if upgrade_kimi_native(log, settings, proxy) {
                return true;
            }
            if kimi_upgrade_plan(resolved.as_deref()) == KimiUpgradePlan::NativeThenNpm {
                push_line(log, "原生安装失败，尝试 npm…");
                return upgrade_via_npm(log, "@moonshot-ai/kimi-code", proxy);
            }
            push_line(
                log,
                "原生升级失败。本机 Kimi 是 ~/.kimi-code 里的单文件，不会改用 npm，以免看起来成功、实际还是旧版。",
            );
            false
        }
        KimiUpgradePlan::NpmOnly => upgrade_via_npm(log, "@moonshot-ai/kimi-code", proxy),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KimiUpgradePlan {
    NativeOnly,
    NativeThenNpm,
    NpmOnly,
}

fn kimi_upgrade_plan(resolved: Option<&Path>) -> KimiUpgradePlan {
    match resolved {
        Some(path) if is_native_kimi(path) => KimiUpgradePlan::NativeOnly,
        Some(_) => KimiUpgradePlan::NpmOnly,
        None => KimiUpgradePlan::NativeThenNpm,
    }
}

fn is_native_kimi(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    text.contains("/.kimi-code/") && (text.ends_with(".exe") || text.ends_with("/kimi"))
}

#[cfg(any(windows, test))]
fn kimi_filehash_fallback_ps() -> &'static str {
    r#"
function Get-FileHash {
  param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string] $Path,
    [string] $Algorithm = 'SHA256'
  )
  $hasher = [System.Security.Cryptography.HashAlgorithm]::Create($Algorithm)
  if (-not $hasher) { throw "unsupported hash algorithm: $Algorithm" }
  $stream = [System.IO.File]::OpenRead($Path)
  try {
    $hash = [BitConverter]::ToString($hasher.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
  } finally {
    $stream.Dispose()
    $hasher.Dispose()
  }
  [pscustomobject]@{ Hash = $hash; Algorithm = $Algorithm; Path = $Path }
}
"#
    .trim()
}

#[cfg(any(windows, test))]
fn kimi_native_install_ps() -> String {
    // Official install.ps1 is a last-resort fallback. Two traps:
    // 1) Invoke-WebRequest progress bars hang for minutes when stdout is
    //    captured (Agent Dock uses Command::output).
    // 2) The catch block calls Read-Host, so a failed install never returns.
    // Define Get-FileHash, silence progress, and make Read-Host a no-op.
    format!(
        "{}\n$ProgressPreference = 'SilentlyContinue'\nfunction Read-Host {{ param($Prompt, $AsSecureString) return '' }}\nInvoke-RestMethod https://code.kimi.com/kimi-code/install.ps1 | Invoke-Expression",
        kimi_filehash_fallback_ps()
    )
}

fn upgrade_kimi_native(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
    // 0.34 Windows `kimi upgrade` prints "Auto-update is not supported on
    // this platform" and tells the user to re-run install.ps1. Download the
    // official binary ourselves so the upgrade is non-interactive.
    let dest = match resolve_binary(ToolId::Kimi, settings) {
        Ok(path) if is_native_kimi(&path) => path,
        _ => default_kimi_bin(),
    };
    match install_kimi_native_binary(log, &dest, proxy) {
        Ok(()) => {
            #[cfg(windows)]
            if let Some(dir) = dest.parent() {
                ensure_user_path(dir, log);
            }
            true
        }
        Err(err) => {
            push_line(log, &err);
            push_line(log, "改走官方安装脚本…");
            upgrade_kimi_native_script(log, settings, proxy)
        }
    }
}

fn default_kimi_bin() -> PathBuf {
    let dir = std::env::var_os("KIMI_INSTALL_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(crate::tools::kimi_home)
        .join("bin");
    if cfg!(windows) {
        dir.join("kimi.exe")
    } else {
        dir.join("kimi")
    }
}

fn kimi_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("win32-x64"),
        ("windows", "aarch64") => Some("win32-arm64"),
        ("macos", "x86_64") => Some("darwin-x64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        _ => None,
    }
}

fn kimi_remote_latest(proxy: &[(String, String)]) -> Result<String, String> {
    let text = http_get_text(KIMI_LATEST_URL, proxy)?;
    parse_kimi_latest(&text)
}

fn parse_kimi_latest(text: &str) -> Result<String, String> {
    parse_version(text).ok_or_else(|| "无法解析 Kimi 最新版本".into())
}

fn parse_kimi_manifest_asset(text: &str, target: &str) -> Result<(String, String), String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("无法解析 Kimi manifest：{err}"))?;
    let entry = value
        .get("platforms")
        .and_then(|platforms| platforms.get(target))
        .ok_or_else(|| format!("manifest 没有 {target}"))?;
    let filename = entry
        .get("filename")
        .and_then(|item| item.as_str())
        .ok_or_else(|| "manifest 缺少 filename".to_string())?;
    let checksum = entry
        .get("checksum")
        .and_then(|item| item.as_str())
        .ok_or_else(|| "manifest 缺少 checksum".to_string())?;
    if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("checksum 无效：{checksum}"));
    }
    Ok((filename.to_string(), checksum.to_ascii_lowercase()))
}

fn install_kimi_native_binary(
    log: &mut String,
    dest: &Path,
    proxy: &[(String, String)],
) -> Result<(), String> {
    let target = kimi_target().ok_or_else(|| "当前系统架构不支持 Kimi 原生安装".to_string())?;
    push_line(log, &format!("下载 Kimi 原生安装包（{target}）"));
    let version = kimi_remote_latest(proxy)?;
    push_line(log, &format!("最新版本 {version}"));
    let manifest_url = format!("{KIMI_DOWNLOAD_BASE}/binaries/{version}/manifest.json");
    let manifest = http_get_text(&manifest_url, proxy)?;
    let (filename, checksum) = parse_kimi_manifest_asset(&manifest, target)?;
    let url = format!("{KIMI_DOWNLOAD_BASE}/binaries/{version}/{filename}");
    push_line(log, &format!("$ GET {url}"));
    push_line(log, "正在下载原生安装包（约 140MB），请稍候…");
    let tmp = dest.with_file_name(format!(
        "{}.download",
        dest.file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kimi".into())
    ));
    let actual = http_download_sha256(&url, proxy, &tmp)?;
    if actual != checksum {
        let size = fs::metadata(&tmp).map(|meta| meta.len()).unwrap_or(0);
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "校验失败：期望 {checksum}，实际 {actual}（{size} 字节）"
        ));
    }
    replace_with_file(dest, &tmp)?;
    push_line(log, &format!("已安装到 {}", dest.display()));
    Ok(())
}

#[cfg(test)]
fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(Sha256::digest(bytes).as_slice())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
fn replace_binary(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建安装目录：{err}"))?;
    }
    let tmp = dest.with_file_name(format!(
        "{}.download",
        dest.file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kimi".into())
    ));
    fs::write(&tmp, bytes).map_err(|err| format!("无法写入安装文件：{err}"))?;
    replace_with_file(dest, &tmp)
}

fn replace_with_file(dest: &Path, src: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建安装目录：{err}"))?;
    }
    if dest.exists() {
        let name = dest
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kimi".into());
        let bak = dest.with_file_name(format!("{name}.bak"));
        if bak.exists() && fs::remove_file(&bak).is_err() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|dur| dur.as_millis())
                .unwrap_or(0);
            let unique = dest.with_file_name(format!("{name}.{stamp}.bak"));
            fs::rename(dest, unique).map_err(|err| format!("无法备份旧文件：{err}"))?;
        } else {
            fs::rename(dest, &bak).map_err(|err| format!("无法备份旧文件：{err}"))?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(src)
            .map_err(|err| format!("无法读取安装文件权限：{err}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(src, perms).map_err(|err| format!("无法设置可执行权限：{err}"))?;
    }
    if fs::rename(src, dest).is_err() {
        fs::copy(src, dest).map_err(|err| format!("无法替换可执行文件：{err}"))?;
        let _ = fs::remove_file(src);
    }
    Ok(())
}

fn proxy_url_from(proxy: &[(String, String)]) -> Option<&str> {
    proxy
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("HTTPS_PROXY") || key.eq_ignore_ascii_case("HTTP_PROXY"))
        .map(|(_, value)| value.as_str())
}

fn http_agent(proxy: &[(String, String)], read_timeout: Duration) -> Result<ureq::Agent, String> {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(read_timeout)
        .timeout_write(Duration::from_secs(30))
        .try_proxy_from_env(false);
    if let Some(url) = proxy_url_from(proxy) {
        let parsed = ureq::Proxy::new(url).map_err(|err| format!("代理地址无法使用：{err}"))?;
        builder = builder.proxy(parsed);
    }
    Ok(builder.build())
}

fn http_get_bytes(url: &str, proxy: &[(String, String)]) -> Result<Vec<u8>, String> {
    match http_get_bytes_once(url, proxy) {
        Ok(bytes) => Ok(bytes),
        Err(err) if !proxy.is_empty() => http_get_bytes_once(url, &[]).map_err(|_| err),
        Err(err) => Err(err),
    }
}

fn http_get_bytes_once(url: &str, proxy: &[(String, String)]) -> Result<Vec<u8>, String> {
    let agent = http_agent(proxy, HTTP_TEXT_READ_TIMEOUT)?;
    let response = agent
        .get(url)
        .set("User-Agent", "agent-dock")
        .set("Accept-Encoding", "identity")
        .call()
        .map_err(|err| format!("下载失败：{err}"))?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(format!("下载失败（{status}）"));
    }
    let mut out = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut out)
        .map_err(|err| format!("读取下载内容失败：{err}"))?;
    Ok(out)
}

pub(crate) fn http_download_file(url: &str, proxy: &[(String, String)], dest: &Path) -> Result<u64, String> {
    match http_download_file_once(url, proxy, dest) {
        Ok(size) => Ok(size),
        Err(err) if !proxy.is_empty() => {
            let _ = fs::remove_file(dest);
            http_download_file_once(url, &[], dest).map_err(|_| err)
        }
        Err(err) => Err(err),
    }
}

fn http_download_file_once(url: &str, proxy: &[(String, String)], dest: &Path) -> Result<u64, String> {
    let hash = http_download_sha256_once(url, proxy, dest)?;
    let _ = hash;
    fs::metadata(dest)
        .map(|meta| meta.len())
        .map_err(|err| format!("无法读取下载文件：{err}"))
}

fn http_download_sha256(url: &str, proxy: &[(String, String)], dest: &Path) -> Result<String, String> {
    match http_download_sha256_once(url, proxy, dest) {
        Ok(hash) => Ok(hash),
        Err(err) if !proxy.is_empty() => {
            let _ = fs::remove_file(dest);
            http_download_sha256_once(url, &[], dest).map_err(|_| err)
        }
        Err(err) => Err(err),
    }
}

fn http_download_sha256_once(
    url: &str,
    proxy: &[(String, String)],
    dest: &Path,
) -> Result<String, String> {
    let agent = http_agent(proxy, HTTP_BIN_READ_TIMEOUT)?;
    let response = agent
        .get(url)
        .set("User-Agent", "agent-dock")
        .set("Accept-Encoding", "identity")
        .call()
        .map_err(|err| format!("下载失败：{err}"))?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(format!("下载失败（{status}）"));
    }
    let declared = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建下载目录：{err}"))?;
    }
    let mut file = fs::File::create(dest).map_err(|err| format!("无法写入下载文件：{err}"))?;
    let mut hasher = Sha256::new();
    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|err| format!("读取下载内容失败：{err}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|err| format!("无法写入下载文件：{err}"))?;
        hasher.update(&buf[..n]);
        written += n as u64;
    }
    let _ = file.flush();
    if let Some(expected) = declared {
        if written != expected {
            return Err(format!("下载不完整：收到 {written} 字节，声明 {expected} 字节"));
        }
    }
    Ok(hex_lower(hasher.finalize().as_slice()))
}

pub(crate) fn http_get_text(url: &str, proxy: &[(String, String)]) -> Result<String, String> {
    let bytes = http_get_bytes(url, proxy)?;
    String::from_utf8(bytes)
        .map(|text| text.trim().to_string())
        .map_err(|_| "远端返回不是文本".into())
}

#[cfg(windows)]
fn ensure_user_path(dir: &Path, log: &mut String) {
    let dir_s = dir.display().to_string().replace('\'', "''");
    let script = format!(
        "$dir = '{dir_s}'; $cur = [Environment]::GetEnvironmentVariable('Path','User'); if (-not $cur) {{ $cur = '' }}; $parts = @($cur.Split(';') | Where-Object {{ $_ }}); if ($parts -notcontains $dir) {{ [Environment]::SetEnvironmentVariable('Path', ($dir + ';' + $cur).TrimEnd(';'), 'User'); Write-Output \"Added $dir to user PATH\" }} else {{ Write-Output \"$dir already in user PATH\" }}"
    );
    match run_capture(
        Path::new("powershell.exe"),
        &["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script],
        &[],
    ) {
        Ok(text) => {
            let line = text.trim();
            if !line.is_empty() {
                push_line(log, line);
            }
        }
        Err(err) => push_line(log, &format!("未能写入用户 PATH：{err}")),
    }
}

fn upgrade_kimi_native_script(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
    #[cfg(windows)]
    {
        let ps = {
            let raw = settings.powershell_path.trim();
            if raw.is_empty() {
                PathBuf::from("powershell.exe")
            } else {
                PathBuf::from(raw)
            }
        };
        let script = kimi_native_install_ps();
        push_line(log, "$ irm https://code.kimi.com/kimi-code/install.ps1 | iex");
        match run_capture_kimi_ps(
            &ps,
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ],
            proxy,
        ) {
            Ok(text) => {
                log.push_str(&text);
                if !text.ends_with('\n') {
                    log.push('\n');
                }
                true
            }
            Err(err) => {
                push_line(log, &err);
                false
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = settings;
        push_line(log, "$ curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash");
        match run_capture(
            Path::new("/bin/bash"),
            &[
                "-lc",
                "curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash",
            ],
            proxy,
        ) {
            Ok(text) => {
                log.push_str(&text);
                if !text.ends_with('\n') {
                    log.push('\n');
                }
                true
            }
            Err(err) => {
                push_line(log, &err);
                false
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UninstallKind {
    Npm(&'static str),
    Binary,
}

fn uninstall_kind(path: &Path, tool: ToolId) -> UninstallKind {
    let text = normalize_path_text(path);
    if text.contains("/npm/") || text.contains("/node_modules/") {
        return UninstallKind::Npm(npm_package(tool));
    }
    UninstallKind::Binary
}

fn uninstall_one(path: &Path, tool: ToolId, log: &mut String) -> bool {
    match uninstall_kind(path, tool) {
        UninstallKind::Npm(pkg) => {
            let npm_ok = uninstall_via_npm(log, pkg);
            let shim_ok = if path.is_file() && is_safe_to_delete(path) {
                remove_binary(path, log)
            } else {
                false
            };
            npm_ok || shim_ok || !path.exists()
        }
        UninstallKind::Binary => {
            if !is_safe_to_delete(path) {
                push_line(
                    log,
                    &format!(
                        "拒绝删除 {}：不在已知用户安装目录。请按官方说明手动卸载。",
                        path.display()
                    ),
                );
                false
            } else {
                remove_binary(path, log)
            }
        }
    }
}

fn uninstall_kimi_official(log: &mut String) -> bool {
    let mut did = false;
    // Official: npm installs are removed with `npm uninstall -g @moonshot-ai/kimi-code`.
    if kimi_npm_present() {
        push_line(
            log,
            "检测到 npm 安装，按官方方式：npm uninstall -g @moonshot-ai/kimi-code",
        );
        did |= uninstall_via_npm(log, "@moonshot-ai/kimi-code");
        for shim in kimi_npm_shims() {
            if shim.is_file() && is_safe_to_delete(&shim) {
                did |= remove_binary(&shim, log);
            }
        }
    }
    // Official: script installs are uninstalled by deleting the kimi executable.
    // Also drop updater leftovers (*.bak). Do not touch sessions/config under ~/.kimi-code.
    for path in kimi_native_cli_files() {
        if path.is_file() && is_safe_to_delete(&path) {
            did |= remove_binary(&path, log);
        }
    }
    did
}

fn kimi_native_cli_files() -> Vec<PathBuf> {
    let dest = default_kimi_bin();
    let dir = dest
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| dest.clone());
    let mut files = vec![dest.clone()];
    if let Some(name) = dest.file_name() {
        files.push(dir.join(format!("{}.bak", name.to_string_lossy())));
        files.push(dir.join(format!("{}.uninstalled", name.to_string_lossy())));
    }
    files.push(dir.join("kimi-legacy.exe"));
    files.push(dir.join("kimi-legacy"));
    files
}

fn npm_global_bin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        #[cfg(windows)]
        {
            home.join("AppData").join("Roaming").join("npm")
        }
        #[cfg(not(windows))]
        {
            home.join(".npm-global").join("bin")
        }
    })
}

fn kimi_npm_shims() -> Vec<PathBuf> {
    let Some(dir) = npm_global_bin_dir() else {
        return Vec::new();
    };
    ["kimi", "kimi.cmd", "kimi.ps1", "kimi.exe"]
        .into_iter()
        .map(|name| dir.join(name))
        .collect()
}

fn kimi_npm_present() -> bool {
    if let Some(dir) = npm_global_bin_dir() {
        if kimi_npm_shims().iter().any(|path| path.is_file()) {
            return true;
        }
        if dir
            .join("node_modules")
            .join("@moonshot-ai")
            .join("kimi-code")
            .exists()
        {
            return true;
        }
    }
    false
}

fn npm_package(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Opencode => "opencode-ai",
        ToolId::Kimi => "@moonshot-ai/kimi-code",
        ToolId::Claude => "@anthropic-ai/claude-code",
        ToolId::Pi => "@earendil-works/pi-coding-agent",
        ToolId::Dsh => "@deepseek-ai/dsh",
        ToolId::Grokbuild => "grok",
    }
}

fn uninstall_via_npm(log: &mut String, pkg: &str) -> bool {
    if find_npm().is_none() {
        log.push_str("未找到 npm，无法卸载。\n");
        return false;
    }
    push_line(log, &format!("$ npm uninstall -g {pkg}"));
    match run_npm(&["uninstall", "-g", pkg], &[]) {
        Ok(text) => {
            log.push_str(&text);
            if !text.ends_with('\n') {
                log.push('\n');
            }
            true
        }
        Err(err) => {
            push_line(log, &err);
            false
        }
    }
}

fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_ascii_lowercase()
}

fn is_safe_to_delete(path: &Path) -> bool {
    let text = normalize_path_text(path);
    let blocked = [
        "/program files/",
        "/program files (x86)/",
        "/windows/",
        "/system32/",
    ];
    if blocked.iter().any(|item| text.contains(item)) {
        return false;
    }
    const ALLOWED: &[&str] = &[
        "/.kimi-code/",
        "/.grok/",
        "/.local/bin/",
        "/.local/share/claude",
        "/appdata/roaming/npm/",
        "/appdata/local/programs/claude",
        "/node_modules/",
        "/.npm/",
        "/.volta/",
        "/.cargo/bin/",
    ];
    ALLOWED.iter().any(|item| text.contains(item))
}

fn remove_binary(path: &Path, log: &mut String) -> bool {
    match fs::remove_file(path) {
        Ok(()) => {
            push_line(log, &format!("已删除 {}", path.display()));
            true
        }
        Err(err) => {
            let name = path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| "cli".into());
            let renamed = path.with_file_name(format!("{name}.uninstalled"));
            match fs::rename(path, &renamed) {
                Ok(()) => {
                    push_line(
                        log,
                        &format!(
                            "文件被占用，已改名为 {}。关掉相关会话后可手动删掉。",
                            renamed.display()
                        ),
                    );
                    true
                }
                Err(_) => {
                    push_line(
                        log,
                        &format!("无法删除 {}：{err}。请先关掉正在运行的会话。", path.display()),
                    );
                    false
                }
            }
        }
    }
}

fn run_npm(args: &[&str], proxy: &[(String, String)]) -> Result<String, String> {
    let npm = find_npm().ok_or_else(|| "未找到 npm，无法查询或升级。请先安装 Node.js / npm。".to_string())?;
    #[cfg(windows)]
    {
        let mut line = if npm.to_string_lossy().contains(' ') {
            format!("\"{}\"", npm.display())
        } else {
            npm.display().to_string()
        };
        for arg in args {
            line.push(' ');
            line.push_str(arg);
        }
        return run_capture(Path::new("cmd.exe"), &["/d", "/c", &line], proxy);
    }
    #[cfg(not(windows))]
    {
        run_capture(&npm, args, proxy)
    }
}

fn upgrade_grok(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
    let exe = match resolve_binary(ToolId::Grokbuild, settings) {
        Ok(path) => path,
        Err(err) => {
            push_line(log, &err);
            return false;
        }
    };
    push_line(log, &format!("$ {} update", exe.display()));
    match run_capture(&exe, &["update"], proxy) {
        Ok(text) => {
            log.push_str(&text);
            if !text.ends_with('\n') {
                log.push('\n');
            }
            true
        }
        Err(err) => {
            push_line(log, &err);
            false
        }
    }
}

fn find_npm() -> Option<PathBuf> {
    ["npm", "npm.cmd"]
        .into_iter()
        .find_map(platform::which_cmd)
}

pub(crate) fn proxy_pairs(url: &str) -> Vec<(String, String)> {
    let url = url.trim();
    if url.is_empty() {
        return Vec::new();
    }
    proxy_env(true, url).unwrap_or_default()
}

fn upgrade_claude(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
    if let Ok(path) = resolve_binary(ToolId::Claude, settings) {
        push_line(log, &format!("$ {} update", path.display()));
        return match run_capture(&path, &["update"], proxy) {
            Ok(text) => {
                log.push_str(&text);
                if !text.ends_with('\n') {
                    log.push('\n');
                }
                true
            }
            Err(err) => {
                push_line(log, &err);
                false
            }
        };
    }
    #[cfg(windows)]
    {
        push_line(log, "$ irm https://claude.ai/install.ps1 | iex");
        push_line(log, "官方说明：https://code.claude.com/docs/en/quickstart");
        match run_capture_kimi_ps(
            Path::new("powershell.exe"),
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "Invoke-RestMethod https://claude.ai/install.ps1 | Invoke-Expression",
            ],
            proxy,
        ) {
            Ok(text) => {
                log.push_str(&text);
                if !text.ends_with('\n') {
                    log.push('\n');
                }
                true
            }
            Err(err) => {
                push_line(log, &err);
                false
            }
        }
    }
    #[cfg(not(windows))]
    {
        push_line(log, "$ curl -fsSL https://claude.ai/install.sh | bash");
        push_line(log, "官方说明：https://code.claude.com/docs/en/quickstart");
        match run_capture(
            Path::new("bash"),
            &["-lc", "curl -fsSL https://claude.ai/install.sh | bash"],
            proxy,
        ) {
            Ok(text) => {
                log.push_str(&text);
                if !text.ends_with('\n') {
                    log.push('\n');
                }
                true
            }
            Err(err) => {
                push_line(log, &err);
                false
            }
        }
    }
}

fn run_capture_kimi_ps(exe: &Path, args: &[&str], proxy: &[(String, String)]) -> Result<String, String> {
    // Keep the default PSModulePath so Invoke-RestMethod can autoload.
    // Get-FileHash is provided by the fallback function in the script.
    run_capture_with(exe, args, proxy, &[])
}

fn run_capture(exe: &Path, args: &[&str], proxy: &[(String, String)]) -> Result<String, String> {
    run_capture_with(exe, args, proxy, &[])
}

fn run_capture_with(
    exe: &Path,
    args: &[&str],
    proxy: &[(String, String)],
    clear_env: &[&str],
) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    platform::prepare_command(&mut cmd);
    for (key, value) in proxy {
        cmd.env(key, value);
    }
    for key in clear_env {
        cmd.env_remove(key);
    }
    cmd.env("NO_COLOR", "1");
    cmd.env("FORCE_COLOR", "0");
    platform::apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法启动 {}：{err}", exe.display()))?;
    let stdout = decode_command_output(&output.stdout);
    let stderr = decode_command_output(&output.stderr);
    let mut text = String::new();
    text.push_str(&stdout);
    if !stderr.trim().is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
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

fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        if !text.chars().any(|c| c == '\u{FFFD}') {
            return text.to_string();
        }
    }
    decode_windows_text(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

fn decode_windows_text(bytes: &[u8]) -> Option<String> {
    #[cfg(windows)]
    {
        use std::ptr;
        extern "system" {
            fn MultiByteToWideChar(
                code_page: u32,
                flags: u32,
                bytes: *const u8,
                byte_len: i32,
                wide: *mut u16,
                wide_len: i32,
            ) -> i32;
        }
        const CP_GBK: u32 = 936;
        unsafe {
            let needed = MultiByteToWideChar(
                CP_GBK,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                ptr::null_mut(),
                0,
            );
            if needed <= 0 {
                return None;
            }
            let mut wide = vec![0u16; needed as usize];
            let written = MultiByteToWideChar(
                CP_GBK,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                wide.as_mut_ptr(),
                needed,
            );
            if written <= 0 {
                return None;
            }
            return Some(String::from_utf16_lossy(&wide[..written as usize]));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = bytes;
        None
    }
}

fn push_line(log: &mut String, line: &str) {
    log.push_str(line);
    if !line.ends_with('\n') {
        log.push('\n');
    }
}

pub fn parse_version(text: &str) -> Option<String> {
    first_semver(text)
}

fn first_semver(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            if let Some((ver, end)) = take_semver(&text[i..]) {
                let _ = end;
                return Some(ver);
            }
        }
        i += 1;
    }
    None
}

fn take_semver(text: &str) -> Option<(String, usize)> {
    let mut parts = Vec::new();
    let mut rest = text;
    for _ in 0..3 {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        parts.push(digits.clone());
        rest = &rest[digits.len()..];
        if parts.len() < 3 {
            if !rest.starts_with('.') {
                return None;
            }
            rest = &rest[1..];
        }
    }
    let mut end = text.len() - rest.len();
    if rest.starts_with('-') {
        let pre: String = rest[1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        if !pre.is_empty() && pre.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) {
            let trimmed = pre.trim_end_matches(['.', '-']);
            if !trimmed.is_empty() {
                end += 1 + trimmed.len();
            }
        }
    }
    Some((text[..end].to_string(), end))
}

fn parse_remote_from_check(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if let Some(idx) = lower.find("latest") {
        if let Some(ver) = first_semver(&text[idx..]) {
            return Some(ver);
        }
    }
    first_semver(text)
}

pub fn compare_label(local: &str, latest: &str) -> String {
    if local == "未安装" {
        return "未安装".into();
    }
    if matches!(local, "异常" | "未知") || matches!(latest, "查询失败" | "需要 npm 才能查询" | "未安装" | "—") {
        return "无法对比".into();
    }
    match (semver_tuple(local), semver_tuple(latest)) {
        (Some(a), Some(b)) if ver_lt(&a, &b) => "可更新".into(),
        (Some(a), Some(b)) if ver_lt(&b, &a) => "新于远端".into(),
        (Some(_), Some(_)) => "已是最新".into(),
        _ => "无法对比".into(),
    }
}

fn ver_lt(
    a: &(u64, u64, u64, Option<String>),
    b: &(u64, u64, u64, Option<String>),
) -> bool {
    let core_a = (a.0, a.1, a.2);
    let core_b = (b.0, b.1, b.2);
    if core_a != core_b {
        return core_a < core_b;
    }
    match (&a.3, &b.3) {
        (None, None) => false,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (Some(left), Some(right)) => pre_lt(left, right),
    }
}

fn pre_lt(a: &str, b: &str) -> bool {
    let pa: Vec<&str> = a.split('.').collect();
    let pb: Vec<&str> = b.split('.').collect();
    let n = pa.len().max(pb.len());
    for i in 0..n {
        match (pa.get(i), pb.get(i)) {
            (None, Some(_)) => return true,
            (Some(_), None) => return false,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(n), Ok(m)) => n.cmp(&m),
                    _ => (*x).cmp(*y),
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord == std::cmp::Ordering::Less;
                }
            }
            (None, None) => {}
        }
    }
    false
}

fn semver_tuple(text: &str) -> Option<(u64, u64, u64, Option<String>)> {
    let ver = first_semver(text)?;
    let (core, pre) = match ver.split_once('-') {
        Some((core, pre)) => (core, Some(pre.to_string())),
        None => (ver.as_str(), None),
    };
    let mut it = core.split('.');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        pre,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_semver() {
        assert_eq!(parse_version("opencode 1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(parse_version("v0.2.101\n").as_deref(), Some("0.2.101"));
        assert_eq!(parse_version("0.1.5-rc.1\n").as_deref(), Some("0.1.5-rc.1"));
        assert_eq!(
            parse_version("@deepseek-ai/dsh@0.1.5-rc.1").as_deref(),
            Some("0.1.5-rc.1")
        );
    }

    #[test]
    fn compares_local_and_latest() {
        assert_eq!(compare_label("1.0.0", "1.0.1"), "可更新");
        assert_eq!(compare_label("1.2.0", "1.2.0"), "已是最新");
        assert_eq!(compare_label("未安装", "1.0.0"), "未安装");
        assert_eq!(compare_label("1.0.0", "查询失败"), "无法对比");
        assert_eq!(compare_label("0.1.5-rc.1", "0.1.5-rc.1"), "已是最新");
        assert_eq!(compare_label("0.1.5-rc.1", "0.1.5-rc.2"), "可更新");
        assert_eq!(compare_label("0.1.5-rc.1", "0.1.5"), "可更新");
        assert_eq!(compare_label("未知", "0.1.5"), "无法对比");
    }

    #[test]
    fn reads_dsh_version_from_npm_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_dir = dir.path().join("node_modules").join("@deepseek-ai").join("dsh");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("package.json"),
            r#"{"name":"@deepseek-ai/dsh","version":"0.1.5-rc.1"}"#,
        )
        .unwrap();
        let shim = dir.path().join("dsh.cmd");
        fs::write(&shim, "@echo off\n").unwrap();
        assert_eq!(
            version_from_npm_layout(&shim, "@deepseek-ai/dsh").as_deref(),
            Some("0.1.5-rc.1")
        );
    }

    #[test]
    fn version_tools_one_or_all() {
        assert_eq!(version_tools(Some(ToolId::Dsh)), vec![ToolId::Dsh]);
        assert_eq!(version_tools(None).len(), 6);
        assert!(version_tools(None).contains(&ToolId::Dsh));
    }

    #[test]
    fn official_urls_are_https() {
        for tool in [
            ToolId::Opencode,
            ToolId::Grokbuild,
            ToolId::Kimi,
            ToolId::Claude,
            ToolId::Pi,
            ToolId::Dsh,
        ] {
            assert!(official_url(tool).starts_with("https://"));
        }
        assert!(official_url(ToolId::Claude).contains("claude.com"));
        assert!(official_url(ToolId::Pi).contains("pi.dev"));
        assert!(official_url(ToolId::Dsh).contains("deepseek"));
    }

    #[test]
    fn reads_latest_from_check_output() {
        assert_eq!(
            parse_remote_from_check("current 0.2.1 latest 0.2.8").as_deref(),
            Some("0.2.8")
        );
    }

    #[test]
    fn detects_native_kimi_bin() {
        assert!(is_native_kimi(Path::new(
            r"C:\Users\me\.kimi-code\bin\kimi.exe"
        )));
        assert!(is_native_kimi(Path::new("/Users/me/.kimi-code/bin/kimi")));
        assert!(!is_native_kimi(Path::new(
            r"C:\Users\me\AppData\Roaming\npm\kimi.cmd"
        )));
        assert!(!is_native_kimi(Path::new("/opt/homebrew/bin/kimi")));
    }

    #[test]
    fn kimi_native_install_ps_covers_missing_filehash() {
        let script = kimi_native_install_ps();
        assert!(
            script.contains("[System.Security.Cryptography.HashAlgorithm]"),
            "must define a .NET SHA256 fallback before the official installer calls Get-FileHash"
        );
        assert!(script.contains("function Get-FileHash"));
        assert!(script.contains("https://code.kimi.com/kimi-code/install.ps1"));
        assert!(
            script.contains("ProgressPreference"),
            "captured Invoke-WebRequest hangs unless progress is silenced"
        );
        assert!(
            script.contains("function Read-Host"),
            "official installer catch block calls Read-Host and would hang Agent Dock"
        );
        assert!(
            !script.contains("Remove-Item Env:PSModulePath"),
            "clearing PSModulePath after PowerShell starts can break Invoke-RestMethod autoload"
        );
    }

    #[test]
    fn hashes_known_bytes() {
        assert_eq!(
            sha256_hex(b"hello\n"),
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        );
        let data = vec![7u8; 80_000];
        let mut hasher = Sha256::new();
        hasher.update(&data);
        assert_eq!(
            hex_lower(hasher.finalize().as_slice()),
            sha256_hex(&data),
            "streaming hash must not hash the digest a second time"
        );
    }

    #[test]
    fn parses_kimi_latest_and_manifest() {
        assert_eq!(parse_kimi_latest("0.42.0\n").as_deref(), Ok("0.42.0"));
        let json = r#"{
            "version": "0.42.0",
            "platforms": {
                "win32-x64": {
                    "filename": "kimi-code-win32-x64.exe",
                    "checksum": "015e9ecfea02a491d47f5d3fb4db47b070c163d25a88ee54fcb6489328117d5d"
                }
            }
        }"#;
        let (name, sum) = parse_kimi_manifest_asset(json, "win32-x64").unwrap();
        assert_eq!(name, "kimi-code-win32-x64.exe");
        assert_eq!(sum.len(), 64);
        assert!(parse_kimi_manifest_asset(json, "linux-x64").is_err());
    }

    #[test]
    fn native_kimi_latest_uses_official_endpoint() {
        assert_eq!(KIMI_LATEST_URL, "https://code.kimi.com/kimi-code/latest");
        assert!(KIMI_DOWNLOAD_BASE.starts_with("https://code.kimi.com"));
    }

    #[test]
    fn kimi_target_matches_host() {
        let target = kimi_target().expect("current OS/arch should have a Kimi native build");
        #[cfg(windows)]
        assert!(target.starts_with("win32-"));
        #[cfg(target_os = "macos")]
        assert!(target.starts_with("darwin-"));
        #[cfg(target_os = "linux")]
        assert!(target.starts_with("linux-"));
    }

    #[test]
    fn kimi_official_uninstall_targets_are_cli_not_sessions() {
        let files = kimi_native_cli_files();
        assert!(
            files.iter().any(|path| {
                let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
                name == "kimi.exe" || name == "kimi"
            }),
            "must include the native kimi executable"
        );
        assert!(
            files.iter().any(|path| path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".bak"))),
            "must include updater leftover .bak"
        );
        assert!(
            files.iter().all(|path| {
                let text = path.to_string_lossy().replace('\\', "/");
                !text.contains("/sessions/") && !text.contains("config.toml")
            }),
            "must not touch session or config data"
        );
        let shims = kimi_npm_shims();
        assert!(shims.iter().any(|path| path.file_name().is_some_and(|name| name == "kimi.cmd" || name == "kimi")));
    }

    #[test]
    fn classifies_uninstall_by_install_source() {
        assert_eq!(
            uninstall_kind(
                Path::new(r"C:\Users\me\AppData\Roaming\npm\opencode.cmd"),
                ToolId::Opencode
            ),
            UninstallKind::Npm("opencode-ai")
        );
        assert_eq!(
            uninstall_kind(
                Path::new(r"C:\Users\me\.kimi-code\bin\kimi.exe"),
                ToolId::Kimi
            ),
            UninstallKind::Binary
        );
        assert_eq!(
            uninstall_kind(Path::new("/Users/me/.grok/bin/grok"), ToolId::Grokbuild),
            UninstallKind::Binary
        );
    }

    #[test]
    fn refuses_to_delete_system_binaries() {
        assert!(!is_safe_to_delete(Path::new(r"C:\Program Files\kimi.exe")));
        assert!(is_safe_to_delete(Path::new(
            r"C:\Users\me\.kimi-code\bin\kimi.exe"
        )));
        assert!(is_safe_to_delete(Path::new(
            r"C:\Users\me\.local\bin\claude.exe"
        )));
        assert!(is_safe_to_delete(Path::new(
            r"C:\Users\me\AppData\Roaming\npm\opencode.cmd"
        )));
        assert!(is_safe_to_delete(Path::new("/Users/me/.grok/bin/grok")));
    }

    #[test]
    fn remove_binary_deletes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kimi.exe");
        std::fs::write(&file, b"x").unwrap();
        let mut log = String::new();
        assert!(remove_binary(&file, &mut log));
        assert!(!file.exists());
        assert!(log.contains("已删除"));
    }

    #[cfg(windows)]
    #[test]
    fn kimi_filehash_fallback_matches_known_digest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("probe.bin");
        std::fs::write(&file, b"hello\n").unwrap();
        let path = file.to_string_lossy().replace('\'', "''");
        let command = format!(
            "{}\n(Get-FileHash -Path '{}' -Algorithm SHA256).Hash",
            kimi_filehash_fallback_ps(),
            path
        );
        let out = run_capture(
            Path::new("powershell.exe"),
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &command],
            &[],
        )
        .expect("powershell should run the .NET Get-FileHash fallback");
        assert!(
            out.to_ascii_lowercase()
                .contains("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"),
            "fallback hash mismatch: {out}"
        );
    }

    #[test]
    fn keeps_utf8_command_output() {
        assert_eq!(decode_command_output(b"1.2.3\n"), "1.2.3\n");
    }

    #[test]
    fn native_kimi_upgrade_does_not_fall_back_to_npm() {
        assert_eq!(
            kimi_upgrade_plan(Some(Path::new(r"C:\Users\me\.kimi-code\bin\kimi.exe"))),
            KimiUpgradePlan::NativeOnly
        );
        assert_eq!(
            kimi_upgrade_plan(Some(Path::new(
                r"C:\Users\me\AppData\Roaming\npm\kimi.cmd"
            ))),
            KimiUpgradePlan::NpmOnly
        );
        assert_eq!(kimi_upgrade_plan(None), KimiUpgradePlan::NativeThenNpm);
    }

    #[test]
    fn replace_binary_backs_up_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("kimi.exe");
        fs::write(&dest, b"old").unwrap();
        replace_binary(&dest, b"new").unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert_eq!(
            fs::read(dest.with_file_name("kimi.exe.bak")).unwrap(),
            b"old"
        );
    }

    #[test]
    #[ignore]
    fn live_install_kimi_native_to_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dest = if cfg!(windows) {
            dir.path().join("kimi.exe")
        } else {
            dir.path().join("kimi")
        };
        let mut log = String::new();
        install_kimi_native_binary(&mut log, &dest, &[]).unwrap_or_else(|err| {
            panic!("native install failed: {err}\n{log}");
        });
        assert!(dest.is_file(), "missing binary: {log}");
        assert!(
            dest.metadata().unwrap().len() > 1_000_000,
            "downloaded file too small: {log}"
        );
    }
}
