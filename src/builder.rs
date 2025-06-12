use crate::config::Config;
use std::{path::{Path, PathBuf}, process::Stdio, fs::File, io::Write};
use cargo_metadata::Metadata;
use thiserror::Error;

pub fn build(config: &Config) -> Result<i32, BuildError> {
    let mut builder = Builder::new(None)?;
    builder.build(&config, &None)
}

pub struct Builder {
    manifest_path: PathBuf,
    project_metadata: Option<Metadata>
}

impl Builder {
    pub fn new(manifest_path: Option<PathBuf>) -> Result<Self, BuildError> {
        let manifest_path = match manifest_path.or_else(|| {
            std::env::var("CARGO_MANIFEST_DIR")
                .ok()
                .map(|dir| Path::new(&dir).join("Cargo.toml"))
        }) {
            Some(path) => path,
            None => {
                println!("WARNING: `CARGO_MANIFEST_DIR` env variable not set");
                locate_cargo_manifest::locate_manifest()?
            }
        };

        Ok(Builder {
            manifest_path,
            project_metadata: None,
        })
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn project_metadata(&mut self) -> Result<&Metadata, cargo_metadata::Error> {
        if let Some(ref metadata) = self.project_metadata {
            return Ok(metadata);
        }
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(&self.manifest_path)
            .exec()?;
        Ok(self.project_metadata.get_or_insert(metadata))
    }
    
    pub fn build(&mut self, config: &Config, bin_path: &Option<PathBuf>) -> Result<i32, BuildError> {
        self.execute_prebuilder(&config)?;
        self.prepare_ovmf_files()?;
        self.prepare_limine_files()?;
        self.copy_kernel(&bin_path)?;
        self.create_limine_iso(&config)?;

        Ok(0)
    }

    fn execute_prebuilder(&self, config: &Config) -> Result<(), BuildError> {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(config.prebuilder.as_ref().unwrap_or(&"None".to_string()))
            .stdout(Stdio::piped())
            .output()
            .map_err(|_| BuildError::CargoBuildFailed)?;
        Ok(())
    }

    fn prepare_ovmf_files(&self) -> Result<(), BuildError> {
        std::fs::create_dir_all("./target/ovmf").unwrap();
        
        self.prepare_ovmf_file(
            &format!("https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-code-{}.fd", "x86_64"), 
            &format!("target/ovmf/ovmf-code-{}.fd", "x86_64")
        )?;
        self.prepare_ovmf_file(
            &format!("https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-vars-{}.fd", "x86_64"),
            &format!("target/ovmf/ovmf-vars-{}.fd", "x86_64")
        )?;
        Ok(())
    }

    fn prepare_ovmf_file(&self, url: &str, path: &str) -> Result<(), BuildError> {
        let response = reqwest::blocking::get(url)
            .map_err(|err| BuildError::DownloadOvmfFirmwareFailed(err))?;
        
        let response = response.error_for_status()
            .map_err(|err| BuildError::DownloadOvmfFirmwareFailed(err))?;

        let bytes = response.bytes()
            .map_err(|err| BuildError::DownloadOvmfFirmwareFailed(err))?;

        let mut file = File::create(path)
            .map_err(|err| BuildError::FileCreationFailed(err))?;
        
        file.write_all(&bytes)
            .map_err(|err| BuildError::FileWriteFailed(err))?;

        Ok(())
    }

    fn prepare_limine_files(&self) -> Result<(), BuildError> {
        std::fs::create_dir_all("./target").unwrap();
        
        self.clone_limine_binary()?;
        self.copy_limine_config()?;
        self.copy_limine_binary()?;
        Ok(())
    }

    fn clone_limine_binary(&self) -> Result<(), BuildError> {
        if !std::path::Path::new("./target/limine").exists() 
            || !std::path::Path::new("./target/limine/limine-bios.sys").exists() 
            || !std::path::Path::new("./target/limine/limine-bios-cd.bin").exists() 
            || !std::path::Path::new("./target/limine/limine-uefi-cd.bin").exists() {
            if !std::path::Path::new("./target/limine").exists() {
                std::fs::create_dir_all("./target/limine").unwrap();
            }

            let branch = "v8.x-binary";
            let url = "https://github.com/limine-bootloader/limine.git";
            let path = Path::new("target/limine");
            let repo = match git2::build::RepoBuilder::new().branch(branch).clone(url, path) {
                Ok(repo) => repo,
                Err(e) => panic!("failed to clone: {}", e),
            };

            let head = repo.head().map_err(|_| BuildError::CloneLimineBinaryFailed)?;
            let head_id = head.target().unwrap();
            let head_commit = repo.find_commit(head_id).map_err(|_| BuildError::CloneLimineBinaryFailed)?;
            println!("Clone limine repo with head_commit: {:?}", head_commit);
        }

        Ok(())
    }

    fn copy_limine_config(&self) -> Result<(), BuildError> {
        std::fs::create_dir_all("./target/iso_root/boot/limine").unwrap();
        std::fs::copy("./limine.conf", "./target/iso_root/boot/limine/limine.conf")
            .map_err(|_| BuildError::CopyLimineConfigFailed)?;
        Ok(())
    }

    fn copy_limine_binary(&self) -> Result<(), BuildError> {
        std::fs::create_dir_all("./target/iso_root/EFI/BOOT").unwrap();
        
        std::fs::copy("target/limine/limine-bios.sys", "target/iso_root/boot/limine/limine-bios.sys")
            .map_err(|e| {
                println!("Failed to copy limine-bios.sys: {}", e);
                BuildError::CopyLimineBinaryFailed
            })?;
        std::fs::copy("target/limine/limine-bios-cd.bin", "target/iso_root/boot/limine/limine-bios-cd.bin")
            .map_err(|e| {
                println!("fila to copy limine-bios-cd.bin: {}", e);
                BuildError::CopyLimineBinaryFailed
            })?;
        std::fs::copy("target/limine/limine-uefi-cd.bin", "target/iso_root/boot/limine/limine-uefi-cd.bin")
            .map_err(|e| {
                println!("Failed to copy limine-uefi-cd.bin: {}", e);
                BuildError::CopyLimineBinaryFailed
            })?;
        
        std::fs::copy("target/limine/BOOTX64.EFI", "target/iso_root/EFI/BOOT/BOOTX64.EFI")
            .map_err(|_| BuildError::CopyLimineBinaryFailed)?;
        std::fs::copy("target/limine/BOOTIA32.EFI", "target/iso_root/EFI/BOOT/BOOTIA32.EFI")
            .map_err(|_| BuildError::CopyLimineBinaryFailed)?;
        Ok(())
    }

    fn copy_kernel(&mut self, bin_path: &Option<PathBuf>) -> Result<(), BuildError> {
        std::fs::create_dir_all("target/iso_root/boot/kernel")
            .map_err(|_| BuildError::CreateDirectoryFailed)?;

        let kernel_binary = if let Some(path) = bin_path {
            path.clone()
        } else {
            PathBuf::from("target/x86_64-unknown-none/debug/kernel")
        };
        
        std::fs::copy(
            &kernel_binary,
            "target/iso_root/boot/kernel/kernel"
        ).map_err(|_| BuildError::CopyKernelBinaryFailed)?;

        Ok(())
    }

    fn create_limine_iso(&self, config: &Config) -> Result<(), BuildError> {
        if let Some(parent) = Path::new(&config.image_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| BuildError::CreateDirectoryFailed)?;
        }

        self.create_raw_iso(&config)?;
        self.install_limine_to_iso(&config)?;
        Ok(())
    }

    fn create_raw_iso(&self, config: &Config) -> Result<(), BuildError> {
        std::process::Command::new("xorriso")
            .arg("-as")
            .arg("mkisofs")
            .arg("-b").arg("boot/limine/limine-bios-cd.bin")
            .arg("-no-emul-boot").arg("-boot-load-size").arg("4").arg("-boot-info-table")
            .arg("--efi-boot").arg("boot/limine/limine-uefi-cd.bin")
            .arg("-efi-boot-part").arg("--efi-boot-image").arg("--protective-msdos-label")
            .arg("target/iso_root")
            .arg("-o").arg(config.image_path.clone())
            .stdout(Stdio::piped())
            .output()
            .map_err(|_| BuildError::CreateLimineIsoFailed)?;
        Ok(())
    }

    fn install_limine_to_iso(&self, config: &Config) -> Result<(), BuildError> {
        std::process::Command::new("target/limine/limine")
            .arg("bios-install")
            .arg(config.image_path.clone())
            .stdout(Stdio::piped())
            .output()
            .map_err(|_| BuildError::InstallLimineToIsoFailed)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("Failed to download OVMF firmware: {0}")]
    DownloadOvmfFirmwareFailed(#[from] reqwest::Error),

    #[error("Failed to clone limine binary file(s)")]
    CloneLimineBinaryFailed,

    #[error("Failed to copy limine.conf")]
    CopyLimineConfigFailed,

    #[error("Failed to copy limine binary file(s)")]
    CopyLimineBinaryFailed,

    #[error("Could not find Cargo.toml file starting from current folder: {0:?}")]
    LocateCargoManifest(#[from] locate_cargo_manifest::LocateManifestError),
    
    #[error("Failed to build the kernel through cargo")]
    CargoBuildFailed,

    #[error("Failed to create the Limine ISO")]
    CreateLimineIsoFailed,

    #[error("Failed to copy kernel binary")]
    CopyKernelBinaryFailed,

    #[error("Failed to create empty image")]
    CreateEmptyImgFailed,

    #[error("Failed to format filesystem image")]
    FormatImgFailed,

    #[error("Failed to add directory to filesystem image")]
    AddImgDirectoryFailed,

    #[error("Failed to add content to filesystem image")]
    AddImgContentFailed,

    #[error("Failed to create directory")]
    CreateDirectoryFailed,

    #[error("Failed to install Limine to the ISO")]
    InstallLimineToIsoFailed,

    #[error("Failed to retrieve cargo metadata")]
    CargoMetadataFailed(#[from] cargo_metadata::Error),

    #[error("Failed to create file: {0}")]
    FileCreationFailed(#[from] std::io::Error),

    #[error("Failed to write to file: {0}")]
    FileWriteFailed(std::io::Error),

    #[error("Limine binary file {0} is missing")]
    LimineBinaryMissing(String),
}