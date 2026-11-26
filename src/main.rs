use std::io::Write;
use std::path::PathBuf;

mod args;
mod cmd;
mod crypto;
mod error;
mod file;
mod models;
mod otp;
mod otpauth;
mod ui;

use args::{Cli, Commands};
use clap::{CommandFactory, Parser};
use error::EXIT_PARTIAL;

fn main() {
    let cli = Cli::parse();

    let codex_path = match resolve_codex_path(&cli) {
        Ok(path) => path,
        Err(e) => fail(e),
    };

    if let Err(e) = run(cli.command, codex_path) {
        fail(e);
    }
}

fn fail(error: error::AppError) -> ! {
    if error.exit_code == EXIT_PARTIAL {
        eprintln!("Warning: {}", error.message);
    } else {
        eprintln!("Error: {}", error.message);
    }
    std::process::exit(error.exit_code);
}

fn resolve_codex_path(cli: &Cli) -> Result<PathBuf, error::AppError> {
    // priority 1 => --path
    // priority 2 => env var HERMES_OTP_PATH
    // priority 3 => default location ~/.config/hermes-otp/
    if let Some(path) = &cli.path {
        return Ok(path.clone());
    }
    if let Ok(path) = std::env::var("HERMES_OTP_PATH") {
        return Ok(PathBuf::from(path));
    }
    file::get_default_path().map_err(error::AppError::from)
}

fn run(command: Commands, codex_path: PathBuf) -> Result<(), error::AppError> {
    match command {
        Commands::Add {
            alias,
            code,
            algorithm,
            digits,
            period,
            otpauth: otpauth_uri,
            import: import_file,
            encryption,
        } => {
            // Defaults are applied below so we can tell when the user set them.
            let custom_params = algorithm.is_some() || digits.is_some() || period.is_some();

            if let Some(uri) = otpauth_uri {
                cmd::add_from_otpauth(
                    &codex_path,
                    &uri,
                    &encryption.unencrypt,
                    &encryption.password,
                )?;
            } else if let Some(file_path) = import_file {
                cmd::add_batch(
                    &codex_path,
                    &file_path,
                    &encryption.unencrypt,
                    &encryption.password,
                )?;
            } else if let Some(uri) = code.as_deref().filter(|c| c.starts_with("otpauth://")) {
                // A URI passed via -c already carries its own parameters.
                if custom_params {
                    return Err("cannot combine --algorithm/--digits/--period \
                                with an otpauth:// URI. Nothing was changed."
                        .into());
                }

                if alias.is_some() {
                    return Err("an otpauth:// URI already contains the alias; \
                                do not pass ALIAS as well. Nothing was changed."
                        .into());
                }

                cmd::add_from_otpauth(
                    &codex_path,
                    uri,
                    &encryption.unencrypt,
                    &encryption.password,
                )?;
            } else {
                let alias = alias.ok_or("Error: alias is required")?;
                let code = code.ok_or("Error: code is required")?;

                cmd::add(
                    &codex_path,
                    &alias,
                    &code,
                    algorithm.unwrap_or_default().as_str(),
                    digits.unwrap_or(6),
                    period.unwrap_or(30),
                    &encryption.unencrypt,
                    &encryption.password,
                )?;
            }
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

        Commands::Passwd {
            password,
            new_password,
        } => {
            cmd::passwd(&codex_path, &password, &new_password)?;
        }

        Commands::Rename {
            old_alias,
            new_alias,
            password,
        } => {
            cmd::rename(&codex_path, &old_alias, &new_alias, &password)?;
        }

        Commands::Ls {
            alias,
            quiet,
            format,
            encryption,
            exact,
            fuzzy,
        } => {
            cmd::ls(
                &codex_path,
                &alias,
                encryption.unencrypt,
                &encryption.password,
                &format,
                quiet,
                exact,
                fuzzy,
            )?;
        }

        Commands::Config {} => {
            codex_path
                .exists()
                .then(|| println!("{}", codex_path.display()))
                .ok_or_else(|| format!("Codex file does not exists at {}", codex_path.display()))?;
        }

        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let bin_name = command.get_name().to_string();
            let mut script = Vec::new();
            clap_complete::aot::generate(shell, &mut command, bin_name, &mut script);

            std::io::stdout()
                .write_all(&script)
                .map_err(|e| format!("could not write completions: {e}"))?;
        }

        Commands::Export { output } => {
            cmd::export(&codex_path, &output)?;
        }

        Commands::Import { input, password } => {
            cmd::import(&codex_path, &input, &password)?;
        }
    }
    Ok(())
}
