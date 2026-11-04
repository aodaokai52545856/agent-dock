//! Zero-model-call smoke test. Pass an isolated config-root dirname under home.
//! cargo run --example omp_rpc_smoke -- <config-root-name>
#![cfg_attr(windows, windows_subsystem = "windows")]
use omp_rpc::{ClientOptions, GetStateCommand};
use std::{path::PathBuf, process::Command};
#[path = "../src/omp_startup.rs"]
mod omp_startup;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dirname = std::env::args()
        .nth(1)
        .ok_or("pass an isolated PI_CONFIG_DIR dirname under home")?;
    let home = dirs::home_dir().ok_or("missing home")?.join(&dirname);
    let agent = home.join("agent");
    std::fs::create_dir_all(&agent)?;
    let exe = std::env::var_os("OMP_SMOKE_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("omp"));
    let setup = std::env::var_os("OMP_SMOKE_SETUP").is_some();
    let factory = |_| {
    let mut cmd = Command::new(&exe);
    cmd.args([
        "--profile",
        "default",
        "--mode",
        "rpc-ui",
        "--no-session",
        "--no-extensions",
        "--no-skills",
        "--no-context-files",
        "--no-lsp",
    ])
    .env("PI_CONFIG_DIR", &dirname)
    .env("PI_CODING_AGENT_DIR", &agent);
    if !setup {
        cmd.args(["--model", "openai/gpt-4.1"])
            .env("OPENAI_API_KEY", "test-not-a-real-key");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    Ok(cmd)
    };
    let (client, _events, setup_required) = omp_startup::connect(factory, ClientOptions::default())?;
    assert_eq!(setup_required, setup);
    let _state = client.call(&GetStateCommand {})?;
    if setup {
        let providers = client.request_value(serde_json::json!({"type":"get_login_providers"}), std::time::Duration::from_secs(10))?;
        assert!(providers.data.is_some());
    }
    println!(
        "OMP protocol {}: ready + get_state + setup/login passed; no model request sent",
        client.protocol_version()
    );
    client.close()?;
    Ok(())
}
