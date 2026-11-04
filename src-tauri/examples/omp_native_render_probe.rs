//! Isolated native OMP PTY bridge for terminal rendering regression checks.
//! Never reads the user's OMP configuration and never sends a model prompt.
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    sync::mpsc,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::path::PathBuf::from(std::env::var_os("OMP_RENDER_FIXTURE").ok_or("missing fixture")?);
    let agent = root.join("agent");
    std::fs::create_dir_all(&agent)?;
    std::fs::write(agent.join("config.yml"), "{\"autoResume\":false}")?;
    let exe = std::env::var("OMP_SMOKE_EXE")?;
    let pair = NativePtySystem::default().openpty(PtySize {
        rows: 32,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let args = [
        "--profile",
        "default",
        "--no-title",
        "--no-extensions",
        "--no-skills",
        "--no-context-files",
        "--no-lsp",
        "--session-dir",
    ];
    let shell = std::env::var_os("OMP_RENDER_SHELL");
    let mut cmd = if let Some(shell) = shell {
        let mut cmd = CommandBuilder::new(shell);
        let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let launch = format!(
            "& {} {} {}",
            quote(&exe),
            args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "),
            quote(&agent.join("sessions").to_string_lossy())
        );
        cmd.args(["-NoLogo", "-NoProfile", "-NoExit", "-Command"]);
        cmd.arg(format!("[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); {launch}"));
        cmd
    } else {
        let mut cmd = CommandBuilder::new(&exe);
        cmd.args(args);
        cmd.arg(agent.join("sessions"));
        cmd
    };
    cmd.cwd(&agent);
    cmd.env(
        "PI_CONFIG_DIR",
        root.strip_prefix(dirs::home_dir().ok_or("missing home")?)?,
    );
    cmd.env("PI_CODING_AGENT_DIR", &agent);
    cmd.env("OMP_DAEMON_IDLE_GRACE_MS", "0");
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "xterm.js");
    cmd.env("TERM_PROGRAM_VERSION", "5.5.0");
    cmd.env("COLORFGBG", "15;0");
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let (tx, rx) = mpsc::channel();
    let input_tx = tx.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if input_tx.send((true, line.into_bytes())).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut buf = [0; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if tx.send((false, buf[..n].to_vec())).is_err() {
                break;
            }
        }
    });
    let mut pending = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(600)) {
            Ok((true, bytes)) => {
                let request: Value = serde_json::from_slice(&bytes)?;
                if request["stop"] == true {
                    break;
                }
                if let Some(data) = request["data"].as_str() {
                    writer.write_all(data.as_bytes())?;
                    writer.flush()?;
                }
                if let (Some(cols), Some(rows)) =
                    (request["cols"].as_u64(), request["rows"].as_u64())
                {
                    pair.master.resize(PtySize {
                        cols: cols as u16,
                        rows: rows as u16,
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;
                }
            }
            Ok((false, bytes)) => {
                pending.extend(bytes);
                let valid = match std::str::from_utf8(&pending) {
                    Ok(_) => pending.len(),
                    Err(e) => e.valid_up_to(),
                };
                if valid > 0 {
                    let data = String::from_utf8_lossy(&pending[..valid]);
                    println!("{}", json!({"data":data}));
                    std::io::stdout().flush()?;
                    pending.drain(..valid);
                }
            }
            Err(_) => break,
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
    Ok(())
}
