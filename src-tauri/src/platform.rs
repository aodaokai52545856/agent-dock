use std::path::{Path, PathBuf};
use std::process::Command;

pub fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell.exe".into()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && Path::new(s).is_file())
            .unwrap_or_else(|| {
                ["/bin/zsh", "/bin/bash", "/bin/sh"]
                    .into_iter()
                    .find(|path| Path::new(path).is_file())
                    .unwrap_or("/bin/zsh")
                    .to_string()
            })
    }
}

pub fn is_windows_shell_name(name: &str) -> bool {
    let file = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    matches!(
        file.to_ascii_lowercase().as_str(),
        "powershell.exe" | "powershell" | "pwsh.exe" | "pwsh" | "cmd.exe" | "cmd"
    )
}

pub fn resolve_shell(configured: &str) -> Result<String, String> {
    let name = configured.trim();
    if name.is_empty() || (!cfg!(windows) && is_windows_shell_name(name)) {
        return Ok(default_shell());
    }
    let path = Path::new(name);
    if path.is_file() {
        return Ok(name.to_string());
    }
    match which_cmd(name) {
        Some(found) => Ok(found.display().to_string()),
        None if cfg!(windows) && is_windows_shell_name(name) => Ok("powershell.exe".into()),
        None => Err(format!(
            "找不到终端：{name}。请确认 Shell 已安装，或清空后使用系统默认。"
        )),
    }
}

pub fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".kimi-code").join("bin"));
        dirs.push(home.join(".grok").join("bin"));
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".cargo").join("bin"));
        dirs.push(home.join(".volta").join("bin"));
        dirs.push(home.join(".asdf").join("shims"));
        #[cfg(unix)]
        {
            dirs.extend(version_manager_bins(&home));
        }
        #[cfg(windows)]
        {
            dirs.push(home.join("AppData").join("Roaming").join("npm"));
            dirs.push(home.join("AppData").join("Local").join("fnm"));
            dirs.push(home.join("AppData").join("Local").join("pi-node").join("current"));
        }
    }
    #[cfg(unix)]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/sbin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/usr/local/sbin"));
    }
    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
    }
    dirs
}

#[cfg(unix)]
fn version_manager_bins(home: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(
        home.join(".local")
            .join("share")
            .join("fnm")
            .join("aliases")
            .join("default")
            .join("bin"),
    );
    dirs.push(home.join(".fnm").join("aliases").join("default").join("bin"));
    let nvm = home.join(".nvm").join("versions").join("node");
    if let Ok(entries) = std::fs::read_dir(nvm) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        versions.sort();
        versions.reverse();
        for version in versions {
            let bin = version.join("bin");
            if bin.is_dir() {
                dirs.push(bin);
            }
        }
    }
    dirs
}

pub fn path_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

pub fn augmented_path() -> String {
    let mut parts: Vec<String> = extra_bin_dirs()
        .into_iter()
        .filter(|path| path.is_dir())
        .map(|path| path.display().to_string())
        .collect();
    if let Ok(existing) = std::env::var("PATH") {
        parts.push(existing);
    }
    parts.join(path_separator())
}

pub fn apply_process_path() {
    std::env::set_var("PATH", augmented_path());
}

pub fn prepare_command(cmd: &mut Command) {
    cmd.env("PATH", augmented_path());
}

pub fn apply_no_window(cmd: &mut Command) {
    let _ = cmd;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

pub fn which_cmd(name: &str) -> Option<PathBuf> {
    if let Ok(path) = which::which(name) {
        return Some(path);
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    for dir in extra_bin_dirs() {
        if !dir.is_dir() {
            continue;
        }
        let direct = dir.join(trimmed);
        if is_executable(&direct) {
            return Some(direct);
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "cmd", "bat"] {
                let candidate = dir.join(format!("{trimmed}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        matches!(
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref(),
            Some("exe" | "cmd" | "bat" | "com")
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for NodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub fn parse_node_version(text: &str) -> Option<NodeVersion> {
    let raw = text.trim().trim_start_matches('v');
    let mut parts = raw.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|item| item.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|item| item.parse().ok()).unwrap_or(0);
    Some(NodeVersion { major, minor, patch })
}

pub fn node_version(exe: &Path) -> Option<NodeVersion> {
    let output = Command::new(exe).arg("-v").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_node_version(&text)
}

pub fn list_node_exes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut push = |path: PathBuf| {
        let key = crate::path_norm::normalize_path(&path.display().to_string());
        if path.is_file() && seen.insert(key) {
            out.push(path);
        }
    };
    if let Ok(iter) = which::which_all("node") {
        for path in iter {
            push(path);
        }
    }
    for dir in extra_bin_dirs() {
        if !dir.is_dir() {
            continue;
        }
        #[cfg(windows)]
        push(dir.join("node.exe"));
        #[cfg(not(windows))]
        {
            let candidate = dir.join("node");
            if is_executable(&candidate) {
                push(candidate);
            }
        }
    }
    out
}

pub fn resolve_node(min: NodeVersion) -> Result<(PathBuf, NodeVersion), String> {
    let mut found = Vec::new();
    for exe in list_node_exes() {
        if let Some(version) = node_version(&exe) {
            found.push((version, exe));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    if let Some((version, exe)) = found.iter().find(|(version, _)| *version >= min) {
        return Ok((exe.clone(), *version));
    }
    let summary = if found.is_empty() {
        "没有找到 Node.js。".into()
    } else {
        found
            .iter()
            .map(|(version, exe)| format!("{version}（{}）", exe.display()))
            .collect::<Vec<_>>()
            .join("、")
    };
    Err(format!(
        "DeepSeek Harness 需要 Node {min} 或更高。当前是 {summary}。低于这个版本时，在终端里执行 dsh web 会立刻回到提示符，浏览器访问 127.0.0.1:3080 也会被拒绝。请升级 Node，或把更新的 node.exe 放到 PATH 前面。"
    ))
}

pub fn path_with_prepend(dir: &Path, path: &str) -> String {
    let prefix = dir.display().to_string();
    if path.is_empty() {
        return prefix;
    }
    format!("{prefix}{}{path}", path_separator())
}

pub fn posix_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_quotes_spaces_and_apostrophes() {
        assert_eq!(posix_single_quote("hello"), "'hello'");
        assert_eq!(posix_single_quote("it's"), "'it'\\''s'");
        assert_eq!(posix_single_quote("/Users/me/My Project"), "'/Users/me/My Project'");
    }

    #[test]
    fn default_shell_is_platform_native() {
        #[cfg(windows)]
        assert_eq!(default_shell(), "powershell.exe");
        #[cfg(not(windows))]
        assert!(!default_shell().ends_with(".exe"));
    }

    #[test]
    fn unix_ignores_copied_windows_shell_setting() {
        #[cfg(not(windows))]
        {
            let resolved = resolve_shell("powershell.exe").unwrap();
            assert!(!is_windows_shell_name(&resolved) || resolved == default_shell());
            assert_eq!(resolved, default_shell());
        }
        #[cfg(windows)]
        {
            assert_eq!(resolve_shell("").unwrap(), "powershell.exe");
        }
    }

    #[test]
    fn recognizes_windows_shell_names() {
        assert!(is_windows_shell_name("powershell.exe"));
        assert!(is_windows_shell_name("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"));
        assert!(!is_windows_shell_name("/bin/zsh"));
    }

    #[test]
    fn parses_node_version_strings() {
        assert_eq!(
            parse_node_version("v22.14.0"),
            Some(NodeVersion {
                major: 22,
                minor: 14,
                patch: 0
            })
        );
        assert_eq!(
            parse_node_version("22.23.1\n"),
            Some(NodeVersion {
                major: 22,
                minor: 23,
                patch: 1
            })
        );
        assert!(parse_node_version("nope").is_none());
        assert!(
            NodeVersion {
                major: 22,
                minor: 23,
                patch: 1
            } >= NodeVersion {
                major: 22,
                minor: 15,
                patch: 0
            }
        );
        assert!(
            NodeVersion {
                major: 22,
                minor: 14,
                patch: 0
            } < NodeVersion {
                major: 22,
                minor: 15,
                patch: 0
            }
        );
    }

    #[test]
    fn prepends_node_dir_ahead_of_path() {
        #[cfg(windows)]
        {
            let next = path_with_prepend(Path::new(r"C:\Users\me\pi-node\current"), r"D:\nodejs;C:\Windows");
            assert!(next.starts_with(r"C:\Users\me\pi-node\current;"));
        }
        #[cfg(not(windows))]
        {
            let next = path_with_prepend(Path::new("/opt/node/bin"), "/usr/bin:/bin");
            assert!(next.starts_with("/opt/node/bin:"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn extra_bin_lookup_skips_extensionless_unix_shim() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("kimi");
        std::fs::write(&shim, b"#!/usr/bin/env node\n").unwrap();
        assert!(
            !is_executable(&shim),
            "Windows must not treat npm's extensionless unix shim as the CLI"
        );
        let cmd = dir.path().join("kimi.cmd");
        std::fs::write(&cmd, b"@echo off\n").unwrap();
        assert!(is_executable(&cmd));
        let exe = dir.path().join("kimi.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        assert!(is_executable(&exe));
    }
}
