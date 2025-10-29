use anyhow::Result;
use jiffs::{config::Config, git::GitDiff, parse_args, validator::Validator};

fn main() -> Result<()> {
    let args = parse_args();

    // Load configuration
    let config = Config::from_file(&args.policy)?;
    println!("Loaded {} rule(s) from policy file", config.rules.len());

    // Get git diff
    println!("Analyzing changes from base SHA: {}", args.base);
    let git_diff = GitDiff::new(&args.base, &args.only_suffixes)?;

    if args.verbose {
        println!("Found {} changed file(s):", git_diff.changed_files.len());
        for path in git_diff.changed_file_paths() {
            println!("  {}", path);
        }
        println!();
    }

    // Validate changes
    let validator = Validator::new(config);
    let result = validator.validate(&git_diff, args.verbose)?;

    // Output results
    println!("Validation Results:");
    println!("  Files processed: {}", result.files_processed);
    println!("  Files matched rules: {}", result.files_matched);
    println!("  Violations found: {}", result.violations.len());

    if !result.violations.is_empty() {
        println!("\nViolations:");
        for violation in &result.violations {
            println!("  File: {}", violation.file_path);
            println!("    Rule: {}", violation.rule_description);
            println!("    Unauthorized changes:");
            for change in &violation.unauthorized_changes {
                println!("      - {}", change);
            }
            println!();
        }
    }

    if result.is_valid {
        println!("✅ All changes are valid according to the policy rules");
        return Ok(());
    }

    println!("❌ Policy violations found");
    std::process::exit(1);
}
