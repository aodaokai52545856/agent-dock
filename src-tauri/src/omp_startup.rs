//! OMP currently requires a selected model even before its RPC login API starts.
use omp_rpc::{Client, ClientOptions, Event};
use std::{process::Command, sync::mpsc::Receiver};

pub const SETUP_MODEL: &str = "openai/gpt-4.1";

fn missing_model(error: &omp_rpc::Error) -> bool {
    let omp_rpc::Error::Startup { stderr, .. } = error else { return false };
    stderr.lines().any(|line| {
        let line = line.trim();
        line.starts_with("No default model selected.")
            || line == "No models available."
            || line.starts_with("No model available matching enabledModels")
    })
}

/// Only a model-less startup is retried. No prompt or credential is synthesized.
/// The setup model is a process-local selector, never a persisted default.
pub fn connect(
    mut command: impl FnMut(bool) -> Result<Command, String>,
    options: ClientOptions,
) -> Result<(Client, Receiver<Event>, bool), String> {
    match Client::spawn(command(false)?, options.clone()) {
        Ok((client, events)) => Ok((client, events, false)),
        Err(error) if missing_model(&error) => {
            let mut cmd = command(true)?;
            cmd.args(["--model", SETUP_MODEL]);
            Client::spawn(cmd, options)
                .map(|(client, events)| (client, events, true))
                .map_err(|error| format!("OMP 首次配置启动失败：{error}"))
        }
        Err(error) => Err(format!("OMP 启动失败：{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_an_explicit_missing_model_diagnostic_enables_setup() {
        let error = |stderr: &str| omp_rpc::Error::Startup {
            error: Box::new(omp_rpc::Error::Closed), stderr: stderr.into(),
        };
        assert!(missing_model(&error("No default model selected. Use /login")));
        assert!(missing_model(&error("No models available.\nSet an API key")));
        assert!(!missing_model(&error("SQLiteError: unable to open database file")));
        assert!(!missing_model(&omp_rpc::Error::Closed));
    }
}
