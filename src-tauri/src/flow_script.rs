//! Runs the command of a flow's script node and reports its exit code.
//!
//! Commands come from repository files. The frontend shows each one and asks
//! before the first run, so nothing here decides what is allowed to run.
use crate::platform;
use serde::Serialize;
use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const OUTPUT_CAP: usize = 200_000;
const POLL: Duration = Duration::from_millis(100);
// A grandchild can keep a pipe open after the command itself has exited.
const DRAIN: Duration = Duration::from_secs(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    pub canceled: bool,
}

#[derive(Default)]
pub struct Hub {
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Hub {
    pub fn run(&self, run_id: &str, cwd: &Path, command: &str, timeout: Duration) -> Result<ScriptResult, String> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.lock().map_err(|e| e.to_string())?.insert(run_id.to_string(), cancel.clone());
        let result = run_command(cwd, command, timeout, &cancel);
        self.running.lock().map_err(|e| e.to_string())?.remove(run_id);
        result
    }

    pub fn stop(&self, run_id: &str) {
        if let Some(cancel) = self.running.lock().ok().and_then(|map| map.get(run_id).cloned()) {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/D", "/S", "/C"]).raw_arg(format!("\"{command}\""));
    cmd
}

#[cfg(not(windows))]
fn shell(command: &str) -> Command {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(platform::default_shell());
    cmd.args(["-c", command]).process_group(0);
    cmd
}

#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    crate::dsh_web::kill_pid(child.id());
}

#[cfg(not(windows))]
fn kill_tree(child: &mut Child) {
    let _ = Command::new("kill")
        .args(["-9", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn keep_tail(buffer: &mut Vec<u8>) {
    if buffer.len() > OUTPUT_CAP * 2 {
        buffer.drain(..buffer.len() - OUTPUT_CAP);
    }
}

fn pump(mut pipe: impl Read + Send + 'static, buffer: Arc<Mutex<Vec<u8>>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            if let Ok(mut out) = buffer.lock() {
                out.extend_from_slice(&chunk[..read]);
                keep_tail(&mut out);
            }
        }
    })
}

fn collected(buffer: &Mutex<Vec<u8>>) -> String {
    let out = buffer.lock().map(|out| out.clone()).unwrap_or_default();
    let tail = &out[out.len().saturating_sub(OUTPUT_CAP)..];
    String::from_utf8_lossy(tail).trim().to_string()
}

fn run_command(cwd: &Path, command: &str, timeout: Duration, cancel: &AtomicBool) -> Result<ScriptResult, String> {
    let mut cmd = shell(command);
    cmd.current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|err| format!("无法启动脚本：{err}"))?;
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let mut readers = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        readers.push(pump(pipe, buffer.clone()));
    }
    if let Some(pipe) = child.stderr.take() {
        readers.push(pump(pipe, buffer.clone()));
    }
    let started = Instant::now();
    let (exit_code, timed_out, canceled) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status.code(), false, false),
            Ok(None) => {}
            Err(err) => {
                kill_tree(&mut child);
                return Err(format!("脚本状态读取失败：{err}"));
            }
        }
        let canceled = cancel.load(Ordering::SeqCst);
        let timed_out = started.elapsed() >= timeout;
        if canceled || timed_out {
            kill_tree(&mut child);
            let _ = child.kill();
            let _ = child.wait();
            break (None, timed_out, canceled);
        }
        thread::sleep(POLL);
    };
    let drained = Instant::now();
    while readers.iter().any(|reader| !reader.is_finished()) && drained.elapsed() < DRAIN {
        thread::sleep(Duration::from_millis(20));
    }
    Ok(ScriptResult { exit_code, output: collected(&buffer), timed_out, canceled })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(command: &str) -> ScriptResult {
        let dir = tempfile::tempdir().unwrap();
        run_command(dir.path(), command, Duration::from_secs(20), &AtomicBool::new(false)).unwrap()
    }

    #[cfg(windows)]
    const SLOW: &str = "ping -n 30 127.0.0.1 > NUL";
    #[cfg(not(windows))]
    const SLOW: &str = "sleep 30";

    #[test]
    fn a_passing_command_reports_zero_and_its_output() {
        let result = run("echo hello");
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.output, "hello");
        assert!(!result.timed_out && !result.canceled);
    }

    #[test]
    fn a_failing_command_reports_its_exit_code() {
        assert_eq!(run("exit 3").exit_code, Some(3));
    }

    #[test]
    fn chained_commands_stop_at_the_first_failure() {
        let result = run("exit 2 && echo after");
        assert_eq!(result.exit_code, Some(2));
        assert!(!result.output.contains("after"));
    }

    #[test]
    fn quotes_reach_the_command_intact() {
        assert!(run("echo \"a b\"").output.contains("a b"));
    }

    #[test]
    fn stderr_is_collected_with_stdout() {
        assert!(run("echo oops 1>&2").output.contains("oops"));
    }

    #[test]
    fn it_runs_in_the_project_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "x").unwrap();
        #[cfg(windows)]
        let command = "dir /b";
        #[cfg(not(windows))]
        let command = "ls";
        let result = run_command(dir.path(), command, Duration::from_secs(20), &AtomicBool::new(false)).unwrap();
        assert!(result.output.contains("marker.txt"));
    }

    #[test]
    fn a_command_past_its_timeout_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let result = run_command(dir.path(), SLOW, Duration::from_millis(300), &AtomicBool::new(false)).unwrap();
        assert!(result.timed_out);
        assert_eq!(result.exit_code, None);
        assert!(started.elapsed() < Duration::from_secs(15));
    }

    #[test]
    fn stopping_ends_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let hub = Arc::new(Hub::default());
        let stopper = hub.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(400));
            stopper.stop("run");
        });
        let result = hub.run("run", dir.path(), SLOW, Duration::from_secs(20)).unwrap();
        handle.join().unwrap();
        assert!(result.canceled);
        assert!(hub.running.lock().unwrap().is_empty());
    }

    #[test]
    fn a_long_output_keeps_its_tail() {
        let mut out = vec![b'a'; OUTPUT_CAP * 3];
        keep_tail(&mut out);
        assert!(out.len() <= OUTPUT_CAP);
        let buffer = Mutex::new(vec![b'x'; OUTPUT_CAP + 10]);
        assert_eq!(collected(&buffer).len(), OUTPUT_CAP);
    }
}
