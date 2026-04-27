use clap::Parser;
use std::{path::Path, process};
use log::debug;
use limage::{
	builder::Builder,
	cli::{Cli, Commands, RunMode},
	config::LimageConfig,
	runner::Runner,
};

fn main() {
	debug!("{:?}", std::env::vars());
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn is_test_executable(path: &Path) -> bool {
    if let Some(parent) = path.parent() {
        if let Some(dirname) = parent.file_name() {
            if let Some(dirname_str) = dirname.to_str() {
                return dirname_str.starts_with("rustdoctest") || dirname_str == "deps";
            }
        }
    }
    false
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = LimageConfig::load()?;

    config.validate()?;

    match cli.command.unwrap_or(Commands::Build) {
        Commands::Build => {
            let builder = Builder::new(config)?;
            builder.build(None)?;
            Ok(())
        }
        Commands::Run { arch, kernel, mode } => {
            let kernel_path = kernel.as_deref();
            let is_test = kernel_path.map(is_test_executable).unwrap_or(false);

            let kernel = Builder::new(config.clone())?.build(kernel_path)?;

            let mode_name = match mode {
                Some(RunMode::Mode { name }) => Some(name.as_str().to_owned()),
                None => None,
            };

            let runner = Runner::new(kernel, arch, is_test);
            let exit_code = runner.run(mode_name.as_deref())?;
            process::exit(exit_code);
        }
        Commands::Clean => {
            let _ = std::fs::remove_dir_all("target/iso_root");
            let _ = std::fs::remove_dir_all("target/ovmf");
            let _ = std::fs::remove_dir_all("target/limine");
            let _ = std::fs::remove_file(&config.build.image_path);
            Ok(())
        }
    }
}
