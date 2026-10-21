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

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum Algorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha512 => "sha512",
        }
    }
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

const QUIET_LONG_HELP: &str = "Plain output, intended for scripts and pipelines: no countdown bar and no \"Press any key to exit\" prompt.

With a single ALIAS match, prints only the code on one line:
  $ hermes-otp ls -q git
  508183

When multiple records match (or no ALIAS is given), prints the plain
table without the countdown bar.

Does not change the output format: combine with -f json for JSON.";

#[derive(Subcommand)]
pub enum Commands {
    /// Adds code to hermes-otp
    Add {
        /// Alias for the entry (required unless using --otpauth or --import)
        alias: Option<String>,
        /// Secret code or otpauth:// URI (required unless using --otpauth or --import)
        #[clap(short = 'c', long)]
        code: Option<String>,
        /// Hash algorithm [default: sha1]
        #[arg(short = 'a', long, value_enum, conflicts_with_all = ["otpauth", "import"])]
        algorithm: Option<Algorithm>,
        /// Number of digits in the generated code [default: 6]
        #[arg(short = 'd', long, conflicts_with_all = ["otpauth", "import"])]
        digits: Option<u32>,
        /// Time step in seconds [default: 30]
        #[arg(short = 't', long, conflicts_with_all = ["otpauth", "import"])]
        period: Option<u32>,
        /// Import from otpauth:// URI
        #[clap(long, conflicts_with_all = ["code", "alias"])]
        otpauth: Option<String>,
        /// Import multiple otpauth:// URIs from a file (one per line)
        #[clap(long, conflicts_with_all = ["code", "otpauth"])]
        import: Option<String>,
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
        #[arg(
            short,
            long,
            help = "Plain output for scripts: no countdown bar or interactive prompt",
            long_help = QUIET_LONG_HELP
        )]
        quiet: bool,
        /// Match the alias exactly (case-sensitive). Conflicts with --fuzzy.
        /// The default lookup is case-insensitive.
        #[clap(short, long, conflicts_with = "fuzzy", verbatim_doc_comment)]
        exact: bool,
        /// Fuzzy search across aliases (sorted by match score)
        #[clap(short = 'z', long, conflicts_with = "exact")]
        fuzzy: bool,
        #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        #[clap(flatten)]
        encryption: EncryptArgs,
    },
    /// Show location of codex file
    Config {},
    /// Export codex to a JSON file
    Export {
        /// Output file path
        #[arg(short = 'o', long)]
        output: PathBuf,
    },
    /// Import records from a JSON file
    Import {
        /// Input JSON file path
        input: PathBuf,
    },
}
