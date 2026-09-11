use super::{ToolId, resolve_binary};
use crate::platform;
use crate::proxy::proxy_env;
use crate::state::AppSettings;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn list_versions(settings: &AppSettings) -> Vec<ToolVersionInfo> {
    [ToolId::Opencode, ToolId::Grokbuild, ToolId::Kimi]
        .into_iter()
        .map(|tool| version_info(tool, settings))
        .collect()
}

pub fn upgrade_tool(tool: ToolId, settings: &AppSettings) -> UpgradeResult {
    let mut log = String::new();
    let proxy = proxy_pairs(settings);
    let result = match tool {
        ToolId::Opencode => upgrade_via_npm(&mut log, "opencode-ai", &proxy),
        ToolId::Kimi => upgrade_kimi(&mut log, settings, &proxy),
        ToolId::Grokbuild => upgrade_grok(&mut log, settings, &proxy),
    };
    let local = match resolve_binary(tool, settings) {
        Ok(path) => local_version(&path, version_args(tool)),
        Err(_) => "未安装".into(),
    };
    UpgradeResult {
        ok: result,
        log,
        local_version: local,
    }
}

fn version_info(tool: ToolId, settings: &AppSettings) -> ToolVersionInfo {
    let resolved = resolve_binary(tool, settings).ok();
    let local = resolved
        .as_ref()
        .map(|path| local_version(path, version_args(tool)))
        .unwrap_or_else(|| "未安装".into());
    let latest = remote_version(tool, settings, resolved.as_deref());
    let compare = compare_label(&local, &latest);
    ToolVersionInfo {
        tool_id: tool,
        name: tool.display_name().to_string(),
        found: resolved.is_some(),
        path: resolved.map(|path| path.display().to_string()),
        local_version: local,
        latest_version: latest,
        compare,
    }
}

fn version_args(tool: ToolId) -> &'static [&'static str] {
    match tool {
        ToolId::Grokbuild => &["version"],
        ToolId::Opencode | ToolId::Kimi => &["--version"],
    }
}

fn local_version(exe: &Path, args: &[&str]) -> String {
    match run_capture(exe, args, &[]) {
        Ok(text) => parse_version(&text).unwrap_or_else(|| "未知".into()),
        Err(_) => "异常".into(),
    }
}

fn remote_version(tool: ToolId, settings: &AppSettings, exe: Option<&Path>) -> String {
    let proxy = proxy_pairs(settings);
    match tool {
        ToolId::Opencode => npm_view("opencode-ai", &proxy),
        ToolId::Kimi => npm_view("@moonshot-ai/kimi-code", &proxy),
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
    match run_npm(&["view", pkg, "version"], proxy) {
        Ok(text) => parse_version(&text).unwrap_or_else(|| "查询失败".into()),
        Err(_) if !proxy.is_empty() => match run_npm(&["view", pkg, "version"], &[]) {
            Ok(text) => parse_version(&text).unwrap_or_else(|| "查询失败".into()),
            Err(_) => "查询失败".into(),
        },
        Err(_) => "查询失败".into(),
    }
}

fn upgrade_via_npm(log: &mut String, pkg: &str, proxy: &[(String, String)]) -> bool {
    if find_npm().is_none() {
        log.push_str("未找到 npm，无法升级。请先安装 Node.js / npm。\n");
        return false;
    }
    push_line(log, &format!("$ npm install -g {pkg}@latest"));
    match run_npm(&["install", "-g", &format!("{pkg}@latest")], proxy) {
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
    match resolve_binary(ToolId::Kimi, settings) {
        Ok(path) if is_native_kimi(&path) => upgrade_kimi_native(log, settings, proxy),
        Ok(_) => upgrade_via_npm(log, "@moonshot-ai/kimi-code", proxy),
        Err(_) if !cfg!(windows) => upgrade_kimi_native(log, settings, proxy),
        Err(_) => upgrade_via_npm(log, "@moonshot-ai/kimi-code", proxy),
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
    // Official install.ps1 calls Get-FileHash. Windows PowerShell 5.1 often
    // cannot see that cmdlet when it inherits pwsh's PSModulePath (Core-only
    // Microsoft.PowerShell.Utility), and some security tools hide the cmdlet
    // entirely. Define a .NET fallback first, then reset the module path.
    format!(
        "{}\nRemove-Item Env:PSModulePath -ErrorAction SilentlyContinue\nInvoke-RestMethod https://code.kimi.com/kimi-code/install.ps1 | Invoke-Expression",
        kimi_filehash_fallback_ps()
    )
}

fn upgrade_kimi_native(log: &mut String, settings: &AppSettings, proxy: &[(String, String)]) -> bool {
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

fn proxy_pairs(settings: &AppSettings) -> Vec<(String, String)> {
    proxy_env(true, &settings.default_proxy_url).unwrap_or_default()
}

fn run_capture_kimi_ps(exe: &Path, args: &[&str], proxy: &[(String, String)]) -> Result<String, String> {
    run_capture_with(exe, args, proxy, &["PSModulePath"])
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
    Some((parts.join("."), text.len() - rest.len()))
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
        (Some(a), Some(b)) if a < b => "可更新".into(),
        (Some(a), Some(b)) if a > b => "新于远端".into(),
        (Some(_), Some(_)) => "已是最新".into(),
        _ => "无法对比".into(),
    }
}

fn semver_tuple(text: &str) -> Option<(u64, u64, u64)> {
    let ver = first_semver(text)?;
    let mut it = ver.split('.');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_semver() {
        assert_eq!(parse_version("opencode 1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(parse_version("v0.2.101\n").as_deref(), Some("0.2.101"));
    }

    #[test]
    fn compares_local_and_latest() {
        assert_eq!(compare_label("1.0.0", "1.0.1"), "可更新");
        assert_eq!(compare_label("1.2.0", "1.2.0"), "已是最新");
        assert_eq!(compare_label("未安装", "1.0.0"), "未安装");
        assert_eq!(compare_label("1.0.0", "查询失败"), "无法对比");
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
        assert!(script.contains("PSModulePath"));
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
}
