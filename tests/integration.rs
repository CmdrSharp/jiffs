use anyhow::Result;
use jiffs::{
    config::Config,
    git::{ChangeType, FileChange, GitDiff},
    validator::Validator,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_end_to_end_validation_with_allowed_changes() -> Result<()> {
    // Create a temporary rules file
    let rules_content = r#"
rules:
  - match:
    - path: kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/clusters/values/revision
"#;

    let mut rules_file = NamedTempFile::new()?;
    write!(rules_file, "{}", rules_content)?;

    // Load config
    let config = Config::from_file(rules_file.path())?;
    let validator = Validator::new(config);

    // Create a mock GitDiff with an allowed change
    let base_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
spec:
  generators:
  - clusters:
      values:
        revision: 0.19.2
"#;

    let current_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
spec:
  generators:
  - clusters:
      values:
        revision: 0.20.0
"#;

    let mut changed_files = std::collections::HashMap::new();
    changed_files.insert(
        "test.yaml".to_string(),
        FileChange {
            base_content: Some(base_content.to_string()),
            current_content: Some(current_content.to_string()),
            change_type: ChangeType::Modified,
        },
    );

    let git_diff = GitDiff { changed_files };

    // Validate - should pass
    let result = validator.validate(&git_diff, false)?;
    assert!(result.is_valid);
    assert_eq!(result.violations.len(), 0);
    assert_eq!(result.files_matched, 1);

    Ok(())
}

#[test]
fn test_end_to_end_validation_with_unauthorized_changes() -> Result<()> {
    // Create a temporary rules file
    let rules_content = r#"
rules:
  - match:
    - path: kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/values/revision
"#;

    let mut rules_file = NamedTempFile::new()?;
    write!(rules_file, "{}", rules_content)?;

    // Load config
    let config = Config::from_file(rules_file.path())?;
    let validator = Validator::new(config);

    // Create a mock GitDiff with an unauthorized change
    let base_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
metadata:
  name: test
spec:
  generators:
  - clusters:
      values:
        revision: 0.19.2
"#;

    let current_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
metadata:
  name: updated-test
spec:
  generators:
  - clusters:
      values:
        revision: 0.19.2
"#;

    let mut changed_files = std::collections::HashMap::new();
    changed_files.insert(
        "test.yaml".to_string(),
        FileChange {
            base_content: Some(base_content.to_string()),
            current_content: Some(current_content.to_string()),
            change_type: ChangeType::Modified,
        },
    );

    let git_diff = GitDiff { changed_files };

    // Validate - should fail
    let result = validator.validate(&git_diff, false)?;
    assert!(!result.is_valid);
    assert_eq!(result.violations.len(), 1);
    assert_eq!(result.files_matched, 1);

    let violation = &result.violations[0];
    assert_eq!(violation.file_path, "test.yaml");
    assert!(!violation.unauthorized_changes.is_empty());

    Ok(())
}

#[test]
fn test_no_matching_files() -> Result<()> {
    // Create a temporary rules file
    let rules_content = r#"
rules:
  - match:
    - path: kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/values/revision
"#;

    let mut rules_file = NamedTempFile::new()?;
    write!(rules_file, "{}", rules_content)?;

    // Load config
    let config = Config::from_file(rules_file.path())?;
    let validator = Validator::new(config);

    // Create a mock GitDiff with a file that doesn't match the rule
    let current_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: test
"#;

    let mut changed_files = std::collections::HashMap::new();
    changed_files.insert(
        "app.yaml".to_string(),
        FileChange {
            base_content: None,
            current_content: Some(current_content.to_string()),
            change_type: ChangeType::Added,
        },
    );

    let git_diff = GitDiff { changed_files };

    // Validate - should pass (no matching files)
    let result = validator.validate(&git_diff, false)?;
    assert!(result.is_valid);
    assert_eq!(result.violations.len(), 0);
    assert_eq!(result.files_matched, 0);

    Ok(())
}

#[test]
fn test_new_file_allowed() -> Result<()> {
    // Create a temporary rules file
    let rules_content = r#"
rules:
  - match:
    - path: kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/values/revision
"#;

    let mut rules_file = NamedTempFile::new()?;
    write!(rules_file, "{}", rules_content)?;

    // Load config
    let config = Config::from_file(rules_file.path())?;
    let validator = Validator::new(config);

    // Create a mock GitDiff with a new file
    let current_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
metadata:
  name: new-app
spec:
  generators:
  - clusters:
      values:
        revision: 0.19.2
"#;

    let mut changed_files = std::collections::HashMap::new();
    changed_files.insert(
        "new-app.yaml".to_string(),
        FileChange {
            base_content: None,
            current_content: Some(current_content.to_string()),
            change_type: ChangeType::Added,
        },
    );

    let git_diff = GitDiff { changed_files };

    // Validate - should pass (new files are allowed)
    let result = validator.validate(&git_diff, false)?;
    assert!(result.is_valid);
    assert_eq!(result.violations.len(), 0);
    assert_eq!(result.files_matched, 1);

    Ok(())
}
