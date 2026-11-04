use omp_rpc::{Client, ClientOptions, Error};
use std::{process::Command, time::Duration};

#[test]
fn startup_failure_retains_the_server_diagnostic() {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/D", "/C", "echo No default model selected. Use /login 1>&2 & exit /B 1"]);
        cmd.creation_flags(0x08000000);
        cmd
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo 'No default model selected. Use /login' >&2; exit 1"]);
        cmd
    };
    command.env_remove("FORCE_COLOR");
    let error = match Client::spawn(command, ClientOptions {
        ready_timeout: Duration::from_secs(5), ..Default::default()
    }) {
        Ok(_) => panic!("a server without ready must not connect"),
        Err(error) => error,
    };
    assert!(matches!(&error, Error::Startup { stderr, .. } if stderr.contains("No default model selected.")), "{error}");
}
