use clap::Parser;
use std::path::PathBuf;

pub mod config;
pub mod git;
pub mod json_path;
pub mod validator;

#[derive(Parser, Debug)]
#[command(version, about = "Validate git diff changes against policy rules")]
pub struct Args {
    /// Base SHA to diff against
    #[arg(long)]
    pub base: String,
    /// Path to policy YAML
    #[arg(long)]
    pub policy: PathBuf,
    /// Optional: limit to files matching this suffix (repeatable). Example: --only-suffix .yaml --only-suffix .yml
    #[arg(long = "only-suffix")]
    pub only_suffixes: Vec<String>,
    /// Optional: verbose output (prints all changed paths)
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}

/// Parse command line arguments and validate the policy file exists
pub fn parse_args() -> Args {
    let args = Args::parse();

    if !args.policy.exists() || !args.policy.is_file() {
        panic!(
            "Policy file does not exist or is not a file: {:?}",
            args.policy
        );
    }

    args
}
