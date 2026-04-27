use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "limage")]
#[command(about = "A tool for building and running kernels", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Build,
    Run {
		#[arg(value_name = "ARCH")]
		arch: TargetArch,
		#[arg(value_name = "KERNEL")]
        kernel: Option<PathBuf>,
        #[command(subcommand)]
        mode: Option<RunMode>,
    },
    Clean,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum TargetArch {
	Aarch64,
	Ia32,
	LoongArch64,
	Riscv64,
	X64,
}

impl TargetArch {
	pub fn ovmf(&self) -> ovmf_prebuilt::Arch {
		match self {
		    TargetArch::Aarch64 => ovmf_prebuilt::Arch::Aarch64,
			TargetArch::Ia32 => ovmf_prebuilt::Arch::Ia32,
			TargetArch::LoongArch64 => ovmf_prebuilt::Arch::LoongArch64,
			TargetArch::Riscv64 => ovmf_prebuilt::Arch::Riscv64,
			TargetArch::X64 => ovmf_prebuilt::Arch::X64,
		}
	}
}

#[derive(Subcommand)]
pub enum RunMode {
    Mode { name: String },
}
