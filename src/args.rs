use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)] // Read from Cargo.toml
pub struct Cli {
    #[arg(short, long, global = true, help = "Custom path to the codex file")]
    pub path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

#[derive(clap::Args)]
pub struct EncryptArgs {
    /// WARNING: Store the secret in plain text. Use for debugging only.
    #[clap(short = 'u', long, verbatim_doc_comment)]
    pub unencrypt: bool,
    /// WARNING: Using this flag leaves password in shell history.
    #[clap(long, verbatim_doc_comment)]
    pub password: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Adds code to hermes-otp
    Add {
        alias: String,
        #[clap(short = 'c', long)]
        code: String,
        #[clap(flatten)]
        encryption: EncryptArgs,
    },
    /// Remove code from hermes-otp
    Remove { alias: String },
    /// Update code by alias
    Update {
        alias: String,
        #[clap(short = 'c', long)]
        code: String,
        #[clap(flatten)]
        encryption: EncryptArgs,
    },
    /// Rename alias
    Rename {
        old_alias: String,
        new_alias: String,
    },
    /// Get codes for all/alias records
    Ls {
        alias: Option<String>,
        #[clap(short, long)]
        quiet: bool,
        #[clap(short, long)]
        exact: bool,
        #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        #[clap(flatten)]
        encryption: EncryptArgs,
    },
    /// Show location of codex file
    Config {},
}
