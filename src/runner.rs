use crate::config::{ConfigError, LimageConfig};
use std::{process::Command, time::Duration};
use log::debug;
use ovmf_prebuilt::Prebuilt;
use thiserror::Error;
use wait_timeout::ChildExt;
use crate::cli::TargetArch;
use crate::Kernel;

pub struct Runner {
    config: LimageConfig,
	arch: TargetArch,
	prebuilt: Prebuilt,
    is_test: bool,
}

impl Runner {
    pub fn new(kernel: Kernel, arch: TargetArch, is_test: bool) -> Self {
        Self { config: kernel.config, arch, prebuilt: kernel.prebuilt, is_test }
    }

    pub fn run(&self, mode: Option<&str>) -> Result<i32, RunError> {
        let cmd_args =
            self.config
                .get_qemu_command(self.arch, &self.prebuilt, &self.config.build.image_path, self.is_test, mode)?;
        let mut command = Command::new(&cmd_args[0]);
        command.args(&cmd_args[1..]);
		debug!("{}\n\t{}", cmd_args[0], cmd_args[1..].join(&"\n\t"));

        if self.is_test {
            self.handle_test_execution(&mut command)
        } else {
            self.handle_normal_execution(&mut command)
        }
    }

    fn handle_normal_execution(&self, command: &mut Command) -> Result<i32, RunError> {
        let status = command
            .status()
            .map_err(|e| RunError::StartQemu { source: e })?;

        Ok(status.code().unwrap_or(1))
    }

    fn handle_test_execution(&self, command: &mut Command) -> Result<i32, RunError> {
        let mut child = command
            .spawn()
            .map_err(|e| RunError::StartQemu { source: e })?;

        let timeout = Duration::from_secs(self.config.test.timeout_secs.into());
        match child
            .wait_timeout(timeout)
            .map_err(|e| RunError::WaitTimeout { source: e })?
        {
            None => {
                // Timeout occurred
                child.kill().map_err(|e| RunError::KillQemu { source: e })?;
                child.wait().map_err(|e| RunError::WaitQemu { source: e })?;
                Ok(2) // Timeout exit code
            }
            Some(status) => {
                let exit_code = status.code().unwrap_or(1);
                if exit_code == self.config.test.success_exit_code {
                    Ok(0) // Success
                } else {
                    Ok(1) // Failure
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("Configuration error: {source}")]
    Config { #[from] source: ConfigError },
    #[error("Failed to start QEMU: {source}\nMake sure QEMU is installed and available in PATH")]
    StartQemu { source: std::io::Error },
    #[error("Wait timeout error: {source}")]
    WaitTimeout { source: std::io::Error },
    #[error("Failed to kill QEMU process: {source}")]
    KillQemu { source: std::io::Error },
    #[error("Failed to wait for QEMU process: {source}")]
    WaitQemu { source: std::io::Error },
}