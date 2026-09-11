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
        }
    }
    #[cfg(unix)]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/sbin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/usr/local/sbin"));
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
    #[cfg(not(unix))]
    {
        true
    }
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
}
