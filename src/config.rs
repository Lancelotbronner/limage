use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimageConfig {
    #[serde(default = "default_build_config")]
    pub build: BuildConfig,
    #[serde(default = "default_qemu_config")]
    pub qemu: QemuConfig,
    #[serde(default = "default_test_config")]
    pub test: TestConfig,
    #[serde(default)]
    pub modes: HashMap<String, ModeConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_image_path")]
    pub image_path: PathBuf,
    #[serde(default)]
    pub prebuilder: Option<String>,
    #[serde(default)]
    pub filesystem: Option<String>,
    #[serde(default = "default_ovmf_path")]
    pub ovmf_path: PathBuf,
    #[serde(default = "default_limine_path")]
    pub limine_path: PathBuf,
    #[serde(default = "default_iso_root")]
    pub iso_root: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QemuConfig {
    #[serde(default = "default_qemu_binary")]
    pub binary: String,
    #[serde(default = "default_qemu_args")]
    pub base_args: Vec<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestConfig {
    #[serde(default = "default_test_timeout")]
    pub timeout_secs: u32,
    #[serde(default = "default_test_success_code")]
    pub success_exit_code: i32,
    #[serde(default = "default_test_no_reboot")]
    pub no_reboot: bool,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModeConfig {
    #[serde(default)]
    pub args: Vec<String>,
}

fn default_build_config() -> BuildConfig {
    BuildConfig {
        image_path: default_image_path(),
        prebuilder: None,
        filesystem: None,
        ovmf_path: default_ovmf_path(),
        limine_path: default_limine_path(),
        iso_root: default_iso_root(),
    }
}

fn default_qemu_config() -> QemuConfig {
    QemuConfig {
        binary: default_qemu_binary(),
        base_args: default_qemu_args(),
        extra_args: Vec::new(),
    }
}

fn default_test_config() -> TestConfig {
    TestConfig {
        timeout_secs: default_test_timeout(),
        success_exit_code: default_test_success_code(),
        no_reboot: default_test_no_reboot(),
        extra_args: Vec::new(),
    }
}

fn default_image_path() -> PathBuf {
    PathBuf::from("target/kernel.iso")
}

fn default_ovmf_path() -> PathBuf {
    PathBuf::from("target/ovmf")
}

fn default_limine_path() -> PathBuf {
    PathBuf::from("target/limine")
}

fn default_iso_root() -> PathBuf {
    PathBuf::from("target/iso_root")
}

fn default_qemu_binary() -> String {
    "qemu-system-x86_64".to_string()
}

fn default_qemu_args() -> Vec<String> {
    vec![
        "-m".to_string(),
        "2G".to_string(),
        "-cdrom".to_string(),
        "{image}".to_string(),
        "-drive".to_string(),
        "if=pflash,unit=0,format=raw,file={ovmf}/ovmf-code-x86_64.fd,readonly=on".to_string(),
        "-drive".to_string(),
        "if=pflash,unit=1,format=raw,file={ovmf}/ovmf-vars-x86_64.fd".to_string(),
    ]
}

fn default_test_timeout() -> u32 {
    300 // 5 minutes
}

fn default_test_success_code() -> i32 {
    33
}

fn default_test_no_reboot() -> bool {
    true
}

impl LimageConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Path::new("limage_config.toml");

        if config_path.exists() {
            Self::from_file(config_path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::ReadConfig { source: e })?;

        toml::from_str(&content).map_err(|e| ConfigError::ParseConfig { source: e })
    }

    pub fn get_mode_args(&self, mode: &str) -> Result<Vec<String>, ConfigError> {
        self.modes
            .get(mode)
            .map(|m| m.args.clone())
            .ok_or_else(|| ConfigError::ModeNotFound {
                mode: mode.to_string(),
            })
    }

    pub fn get_qemu_command(
        &self,
        image_path: &Path,
        is_test: bool,
        mode: Option<&str>,
    ) -> Result<Vec<String>, ConfigError> {
        let mut cmd = vec![self.qemu.binary.clone()];

        for arg in &self.qemu.base_args {
            cmd.push(
                arg.replace("{image}", &image_path.display().to_string())
                    .replace("{ovmf}", &self.build.ovmf_path.display().to_string()),
            );
        }

        if let Some(mode_name) = mode {
            let mode_args = self.get_mode_args(mode_name)?;
            cmd.extend(mode_args);
        }

        cmd.extend(self.qemu.extra_args.clone());

        // Add test-specific args
        if is_test {
            if self.test.no_reboot {
                cmd.push("-no-reboot".to_string());
            }
            cmd.extend(self.test.extra_args.clone());
        }

        Ok(cmd)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        /*// Create necessary directories if they don't exist
        let dirs = [
            (&self.build.ovmf_path, "OVMF"),
            (&self.build.limine_path, "Limine"),
            (&self.build.iso_root, "ISO root"),
        ];

        for (path, name) in dirs {
            if !path.exists() {
                std::fs::create_dir_all(path).map_err(|e| ConfigError::CreateDirectory {
                    path: path.to_path_buf(),
                    name: name.to_string(),
                    source: e,
                })?;
            }
        }
            */
        Ok(())
    }
}

impl Default for LimageConfig {
    fn default() -> Self {
        Self {
            build: default_build_config(),
            qemu: default_qemu_config(),
            test: default_test_config(),
            modes: HashMap::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file")]
    ReadConfig { source: std::io::Error },
    #[error("Failed to parse config file")]
    ParseConfig { source: toml::de::Error },
    #[error("Failed to create {name} directory at {path:?}")]
    CreateDirectory {
        path: PathBuf,
        name: String,
        source: std::io::Error,
    },
    #[error("Mode '{mode}' not found in configuration file")]
    ModeNotFound { mode: String },
}
