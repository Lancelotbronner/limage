use clap::{Parser, Subcommand};
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
        #[arg(value_name = "KERNEL")]
        kernel: Option<PathBuf>,
        #[command(subcommand)]
        mode: Option<RunMode>,
    },
    Clean,
}

#[derive(Subcommand)]
pub enum RunMode {
    Mode { name: String },
}
