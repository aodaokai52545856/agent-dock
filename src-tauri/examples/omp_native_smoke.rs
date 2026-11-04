//! Opens the real OMP TUI in a ConPTY with isolated configuration. Sends no prompt.
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::{
    io::{Read, Write},
    sync::mpsc,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dirname = std::env::args()
        .nth(1)
        .ok_or("pass isolated config dirname under home")?;
    let agent = dirs::home_dir()
        .ok_or("missing home")?
        .join(&dirname)
        .join("agent");
    std::fs::create_dir_all(&agent)?;
    std::fs::write(
        agent.join("config.yml"),
        "{\"enabledProviders\":[\"openai\"],\"autoResume\":false}",
    )?;
    let exe = std::env::var_os("OMP_SMOKE_EXE").unwrap_or_else(|| "omp".into());
    let pair = NativePtySystem::default().openpty(PtySize {
        rows: 32,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let mut cmd = CommandBuilder::new(exe);
    cmd.args([
        "--profile",
        "default",
        "--model",
        "openai/gpt-4.1",
        "--no-title",
        "--no-extensions",
        "--no-skills",
        "--no-context-files",
        "--no-lsp",
        "--session-dir",
    ]);
    cmd.arg(agent.join("sessions").join("fixture"));
    cmd.cwd(&agent);
    cmd.env("PI_CONFIG_DIR", &dirname);
    cmd.env("PI_CODING_AGENT_DIR", &agent);
    cmd.env("OPENAI_API_KEY", "test-not-a-real-key");
    cmd.env("OMP_DAEMON_IDLE_GRACE_MS", "0");
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(300)) {
            let text = String::from_utf8_lossy(&chunk);
            if text.contains("\x1b[6n") {
                writer.write_all(b"\x1b[1;1R")?;
            }
            if text.contains("\x1b[c") {
                writer.write_all(b"\x1b[?1;2c")?;
            }
            if text.contains("\x1b[>c") {
                writer.write_all(b"\x1b[>0;0;0c")?;
            }
            output.extend(chunk);
            let captured=String::from_utf8_lossy(&output).to_lowercase();
            if captured.contains("gpt-4.1") || captured.contains("ctrl+c exit setup") {
                break;
            }
        }
        if child.try_wait()?.is_some() {
            break;
        }
    }
    #[cfg(windows)]
    if let Some(pid) = child.process_id() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill.exe")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .output();
    }
    let _ = child.kill();
    let _ = child.wait();
    if let Some(path) = std::env::var_os("OMP_SMOKE_OUTPUT") {
        std::fs::write(path, &output)?;
    }
    let text = String::from_utf8_lossy(&output).to_lowercase();
    if !text.contains("gpt-4.1") && !(text.contains("ctrl+c exit setup") && text.contains("type to search")) {
        return Err(format!(
            "native TUI did not render chat or onboarding; captured {} bytes",
            output.len()
        )
        .into());
    }
    if text.contains("\"type\":\"ready\"") {
        return Err("RPC launched instead of native TUI".into());
    }
    println!(
        "PASS: real OMP native TUI rendered chat/onboarding in a PTY ({} bytes); no model request sent",
        output.len()
    );
    Ok(())
}
