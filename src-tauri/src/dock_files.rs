//! Restricted file access for a project's `.agentDock/` directory.
//!
//! Flow orchestration reads contracts and writes artifacts here. Paths come
//! from repository files, so nothing may leave the directory, and writes are
//! limited to the folders Dock owns.
use crate::state;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use tauri::AppHandle;

const ROOT: &str = ".agentDock";
const READ_ROOTS: [&str; 4] = ["flows", "memory", "roles", "tasks"];
const WRITE_ROOTS: [&str; 2] = ["flows", "tasks"];
const MAX_FILE_BYTES: usize = 1 << 20;
// Task documents stay out of "uncommitted changes" reviews. Deleting this line
// puts them under version control.
const GITIGNORE: &str = "tasks/\n";
const WINDOWS_DEVICES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockFile {
    pub text: String,
    pub sha256: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name.contains([':', '<', '>', '"', '|', '?', '*', '\0'])
        && !WINDOWS_DEVICES.contains(&stem.as_str())
}

fn split_path<'a>(rel: &'a str, roots: &[&str]) -> Result<Vec<&'a str>, String> {
    let parts: Vec<&str> = rel.split(['/', '\\']).collect();
    if !parts.iter().all(|part| valid_name(part)) {
        return Err(format!("{ROOT} 路径不合法：{rel}"));
    }
    if !roots.contains(&parts[0]) {
        return Err(format!("{ROOT} 下只能访问 {}：{rel}", roots.join("、")));
    }
    Ok(parts)
}

// A link inside the directory would turn a contained path into any path.
fn reject_links(project: &Path, parts: &[&str]) -> Result<(), String> {
    let mut current = project.join(ROOT);
    for part in std::iter::once(&"").chain(parts.iter()) {
        if !part.is_empty() {
            current.push(part);
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("{ROOT} 内不允许符号链接：{}", current.display()));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(format!("检查 {ROOT} 路径失败：{err}")),
        }
    }
    Ok(())
}

fn locate(project: &Path, rel: &str, roots: &[&str]) -> Result<PathBuf, String> {
    let parts = split_path(rel, roots)?;
    reject_links(project, &parts)?;
    Ok(parts.iter().fold(project.join(ROOT), |path, part| path.join(part)))
}

fn ensure_root(project: &Path) -> Result<(), String> {
    let root = project.join(ROOT);
    fs::create_dir_all(&root).map_err(|err| format!("创建 {ROOT} 失败：{err}"))?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(".gitignore"))
    {
        Ok(mut file) => file
            .write_all(GITIGNORE.as_bytes())
            .map_err(|err| format!("写入 {ROOT}/.gitignore 失败：{err}")),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(format!("创建 {ROOT}/.gitignore 失败：{err}")),
    }
}

fn read_in(project: &Path, rel: &str) -> Result<Option<DockFile>, String> {
    let path = locate(project, rel, &READ_ROOTS)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("读取 {rel} 失败：{err}")),
    };
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("{rel} 超过 1 MiB，无法读取"));
    }
    let sha256 = sha256(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| format!("{rel} 不是 UTF-8 文本"))?;
    Ok(Some(DockFile { text, sha256 }))
}

fn write_in(project: &Path, rel: &str, text: &str) -> Result<String, String> {
    if text.len() > MAX_FILE_BYTES {
        return Err(format!("{rel} 超过 1 MiB，无法写入"));
    }
    let path = locate(project, rel, &WRITE_ROOTS)?;
    ensure_root(project)?;
    let parent = path.parent().expect("located paths have a parent");
    fs::create_dir_all(parent).map_err(|err| format!("创建 {rel} 的目录失败：{err}"))?;
    let tmp = path.with_extension("agentdock-tmp");
    fs::write(&tmp, text)
        .and_then(|_| fs::rename(&tmp, &path))
        .map_err(|err| {
            let _ = fs::remove_file(&tmp);
            format!("写入 {rel} 失败：{err}")
        })?;
    Ok(sha256(text.as_bytes()))
}

fn list_in(project: &Path, dir: &str) -> Result<Vec<String>, String> {
    let path = locate(project, dir, &READ_ROOTS)?;
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("列出 {dir} 失败：{err}")),
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|kind| kind.is_file()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}

fn remove_in(project: &Path, rel: &str) -> Result<(), String> {
    let path = locate(project, rel, &WRITE_ROOTS)?;
    match fs::remove_file(&path) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(format!("删除 {rel} 失败：{err}")),
        _ => Ok(()),
    }
}

fn project_root(app: &AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let state = state::load_state(app)?;
    Ok(PathBuf::from(&state::find_project(&state, project_id)?.path))
}

#[tauri::command]
pub fn dock_file_read(
    app: AppHandle,
    project_id: String,
    path: String,
) -> Result<Option<DockFile>, String> {
    read_in(&project_root(&app, &project_id)?, &path)
}

#[tauri::command]
pub fn dock_file_write(
    app: AppHandle,
    project_id: String,
    path: String,
    text: String,
) -> Result<String, String> {
    write_in(&project_root(&app, &project_id)?, &path, &text)
}

#[tauri::command]
pub fn dock_file_list(
    app: AppHandle,
    project_id: String,
    dir: String,
) -> Result<Vec<String>, String> {
    list_in(&project_root(&app, &project_id)?, &dir)
}

#[tauri::command]
pub fn dock_file_remove(app: AppHandle, project_id: String, path: String) -> Result<(), String> {
    remove_in(&project_root(&app, &project_id)?, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempProject(PathBuf);

    impl TempProject {
        fn new() -> Self {
            let name = format!(
                "dock-files-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_then_read_returns_the_same_hash() {
        let project = TempProject::new();
        let written = write_in(&project.0, "tasks/t1/design.md", "设计").unwrap();
        let read = read_in(&project.0, "tasks/t1/design.md").unwrap().unwrap();
        assert_eq!(read.text, "设计");
        assert_eq!(read.sha256, written);
    }

    #[test]
    fn first_write_ignores_tasks_and_keeps_an_edited_gitignore() {
        let project = TempProject::new();
        write_in(&project.0, "tasks/t1/a.md", "a").unwrap();
        let ignore = project.0.join(ROOT).join(".gitignore");
        assert_eq!(fs::read_to_string(&ignore).unwrap(), GITIGNORE);
        fs::write(&ignore, "").unwrap();
        write_in(&project.0, "flows/f.json", "{}").unwrap();
        assert_eq!(fs::read_to_string(&ignore).unwrap(), "");
    }

    #[test]
    fn missing_files_read_as_none_and_list_as_empty() {
        let project = TempProject::new();
        assert!(read_in(&project.0, "memory/index.md").unwrap().is_none());
        assert!(list_in(&project.0, "flows").unwrap().is_empty());
        assert!(!project.0.join(ROOT).exists());
    }

    #[test]
    fn paths_cannot_leave_the_directory() {
        let project = TempProject::new();
        for rel in [
            "../secret",
            "tasks/../../secret",
            "tasks\\..\\..\\secret",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "tasks//a.md",
            "",
        ] {
            assert!(read_in(&project.0, rel).is_err(), "{rel}");
            assert!(write_in(&project.0, rel, "x").is_err(), "{rel}");
        }
    }

    #[test]
    fn windows_only_escapes_are_refused() {
        let project = TempProject::new();
        for rel in ["tasks/a.md:stream", "tasks/NUL", "tasks/con.txt", "tasks/a.", "tasks/a "] {
            assert!(write_in(&project.0, rel, "x").is_err(), "{rel}");
        }
    }

    #[test]
    fn only_dock_owned_folders_are_writable() {
        let project = TempProject::new();
        assert!(write_in(&project.0, "memory/index.md", "x").is_err());
        assert!(write_in(&project.0, "roles/dev/role.md", "x").is_err());
        assert!(write_in(&project.0, ".gitignore", "x").is_err());
        assert!(remove_in(&project.0, "memory/index.md").is_err());
    }

    #[test]
    fn oversized_text_is_refused() {
        let project = TempProject::new();
        let text = "x".repeat(MAX_FILE_BYTES + 1);
        assert!(write_in(&project.0, "tasks/t1/a.md", &text).is_err());
    }

    #[test]
    fn replacing_a_file_leaves_no_temp_file() {
        let project = TempProject::new();
        write_in(&project.0, "flows/a.json", "1").unwrap();
        write_in(&project.0, "flows/a.json", "2").unwrap();
        assert_eq!(list_in(&project.0, "flows").unwrap(), vec!["a.json"]);
    }

    #[test]
    fn remove_is_idempotent() {
        let project = TempProject::new();
        write_in(&project.0, "flows/a.json", "1").unwrap();
        remove_in(&project.0, "flows/a.json").unwrap();
        remove_in(&project.0, "flows/a.json").unwrap();
        assert!(list_in(&project.0, "flows").unwrap().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn junctions_inside_the_directory_are_refused() {
        let project = TempProject::new();
        let outside = TempProject::new();
        write_in(&project.0, "tasks/t1/a.md", "a").unwrap();
        let status = std::process::Command::new("cmd")
            .arg("/c")
            .arg("mklink")
            .arg("/J")
            .arg(project.0.join(ROOT).join("tasks").join("link"))
            .arg(&outside.0)
            .output()
            .unwrap();
        // Endpoint protection can block cmd.exe from a freshly built test binary.
        if !status.status.success() {
            eprintln!("skipped: mklink is unavailable here");
            return;
        }
        assert!(write_in(&project.0, "tasks/link/a.md", "x").is_err());
        assert!(read_in(&project.0, "tasks/link/a.md").is_err());
        assert!(!outside.0.join("a.md").exists());
    }
}
