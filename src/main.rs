use std::path::PathBuf;

mod args;
mod cmd;
mod file;
mod models;
mod otp;
mod ui;

use args::{Cli, Commands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    let codex_path = resolve_codex_path(&cli);

    if let Err(e) = run(cli.command, codex_path) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn resolve_codex_path(cli: &Cli) -> PathBuf {
    // priority 1 => --path
    // priority 2 => env var HERMES_OTP_PATH
    // priority 3 => default location ~/.config/hermes-otp/
    cli.path
        .clone()
        .or_else(|| std::env::var("HERMES_OTP_PATH").ok().map(PathBuf::from))
        .unwrap_or_else(file::get_default_path)
}

fn run(command: Commands, codex_path: PathBuf) -> Result<(), String> {
    match command {
        Commands::Add {
            alias,
            code,
            encryption,
        } => {
            cmd::add(
                &codex_path,
                &alias,
                &code,
                &encryption.unencrypt,
                &encryption.password,
            )?;
        }

        Commands::Remove { alias } => {
            cmd::remove(&codex_path, &alias)?;
        }

        Commands::Update {
            alias,
            code,
            encryption,
        } => {
            cmd::update_code(
                &codex_path,
                &alias,
                &code,
                &encryption.unencrypt,
                &encryption.password,
            )?;
        }

        Commands::Rename {
            old_alias,
            new_alias,
        } => {
            cmd::rename(&codex_path, &old_alias, &new_alias)?;
        }

        Commands::Ls {
            alias,
            quiet,
            format,
            encryption,
            exact,
        } => {
            cmd::ls(
                &codex_path,
                &alias,
                encryption.unencrypt,
                &encryption.password,
                &format,
                quiet,
                exact,
            )?;
        }

        Commands::Config {} => {
            codex_path
                .exists()
                .then(|| println!("{}", codex_path.display()))
                .ok_or_else(|| format!("Codex file does not exists at {}", codex_path.display()))?;
        }
    }
    Ok(())
}
