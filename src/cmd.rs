use crate::{BriskError, Result};
use std::ffi::OsStr;
use std::process::{Command, Stdio};

pub struct Cmd {
    inner: Command,
}

pub fn command(program: &str) -> Cmd {
    Cmd {
        inner: Command::new(program),
    }
}

impl Cmd {
    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub fn run(&mut self) -> Result<()> {
        let status = self.inner.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(BriskError::Message(format!(
                "command failed with status {status}"
            )))
        }
    }

    pub fn output(&mut self) -> Result<Vec<u8>> {
        let output = self
            .inner
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(command_error(output.status, &output.stdout, &output.stderr))
        }
    }

    pub fn display(&self) -> String {
        format!("{:?}", self.inner).replace('"', "")
    }

    pub fn run_silent(&mut self) -> Result<()> {
        let output = self
            .inner
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(output.status, &output.stdout, &output.stderr))
        }
    }
}

fn command_error(status: std::process::ExitStatus, stdout: &[u8], stderr: &[u8]) -> BriskError {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    BriskError::Message(if detail.is_empty() {
        format!("command failed with status {status}")
    } else {
        detail
    })
}
