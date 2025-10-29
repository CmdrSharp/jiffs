#[cfg(test)]
mod file_operations {
    use anyhow::Result;
    use jiffs::config::Config;
    use jiffs::git::{ChangeType, FileChange, GitDiff};
    use jiffs::validator::Validator;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn deleted_files_matching_rules_cause_validation_failure() -> Result<()> {
        // Create rules that match ApplicationSet files
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
        let config = Config::from_file(rules_file.path())?;
        let validator = Validator::new(config);

        // Create a deleted ApplicationSet file
        let deleted_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: ApplicationSet
metadata:
  name: test-app
spec:
  generators:
  - clusters:
      values:
        revision: 1.0.0
"#;

        let mut changed_files = HashMap::new();
        changed_files.insert(
            "deleted-app.yaml".to_string(),
            FileChange {
                base_content: Some(deleted_content.to_string()),
                current_content: None,
                change_type: ChangeType::Deleted,
            },
        );

        let git_diff = GitDiff { changed_files };

        // Validate - should fail because the deleted file matches a rule
        let result = validator.validate(&git_diff, false)?;

        assert!(
            !result.is_valid,
            "Deleted file matching rule should cause validation failure"
        );
        assert_eq!(
            result.violations.len(),
            1,
            "Should have exactly one violation"
        );
        assert_eq!(result.files_matched, 1, "Should match one file");

        let violation = &result.violations[0];
        assert_eq!(violation.file_path, "deleted-app.yaml");
        assert!(violation.rule_description.contains("prohibits deletion"));
        assert_eq!(violation.unauthorized_changes, vec!["File deletion"]);

        Ok(())
    }

    #[test]
    fn deleted_files_not_matching_rules_pass_validation() -> Result<()> {
        // Create rules that only match ApplicationSet files
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
        let config = Config::from_file(rules_file.path())?;
        let validator = Validator::new(config);

        // Create a deleted Application file (not ApplicationSet)
        let deleted_content = r#"
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: test-app
spec:
  project: default
"#;

        let mut changed_files = HashMap::new();
        changed_files.insert(
            "deleted-app.yaml".to_string(),
            FileChange {
                base_content: Some(deleted_content.to_string()),
                current_content: None,
                change_type: ChangeType::Deleted,
            },
        );

        let git_diff = GitDiff { changed_files };

        // Validate - should pass because the deleted file doesn't match any rule
        let result = validator.validate(&git_diff, false)?;

        assert!(
            result.is_valid,
            "Deleted file not matching rule should pass validation"
        );
        assert_eq!(result.violations.len(), 0, "Should have no violations");
        assert_eq!(result.files_matched, 0, "Should match no files");

        Ok(())
    }
}
