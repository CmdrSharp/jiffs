use anyhow::{Context, Result};
use serde_json::Value;

use crate::config::{Config, Rule};
use crate::git::{ChangeType, GitDiff};
use crate::json_path::JsonPathMatcher;

#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<Violation>,
    pub files_processed: usize,
    pub files_matched: usize,
}

#[derive(Debug)]
pub struct Violation {
    pub file_path: String,
    pub rule_description: String,
    pub unauthorized_changes: Vec<String>,
}

pub struct Validator {
    config: Config,
}

impl Validator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn validate(&self, git_diff: &GitDiff, verbose: bool) -> Result<ValidationResult> {
        let mut violations = Vec::new();
        let mut files_matched = 0;

        for (file_path, file_change) in &git_diff.changed_files {
            if verbose {
                println!("Processing file: {}", file_path);
            }

            // For deleted files, we need to check the base content to see if it would match rules
            let json_for_rule_matching = if file_change.change_type == ChangeType::Deleted {
                match &file_change.base_content {
                    Some(content) => match Self::parse_yaml_or_json(content) {
                        Ok(json) => json,
                        Err(_) => {
                            if verbose {
                                println!("  Skipping non-YAML/JSON deleted file: {}", file_path);
                            }
                            continue;
                        }
                    },
                    None => {
                        if verbose {
                            println!(
                                "  No base content available for deleted file: {}",
                                file_path
                            );
                        }
                        continue;
                    }
                }
            } else {
                let current_content = match &file_change.current_content {
                    Some(content) => content,
                    None => continue,
                };

                match Self::parse_yaml_or_json(current_content) {
                    Ok(json) => json,
                    Err(_) => {
                        if verbose {
                            println!("  Skipping non-YAML/JSON file: {}", file_path);
                        }
                        continue;
                    }
                }
            };

            for rule in &self.config.rules {
                if Self::file_matches_rule(&json_for_rule_matching, rule) {
                    files_matched += 1;

                    if verbose {
                        println!(
                            "  File matches rule with {} match conditions",
                            rule.match_conditions.len()
                        );
                    }

                    if let Some(violation) =
                        self.validate_file_against_rule(file_path, file_change, rule, verbose)?
                    {
                        violations.push(violation);
                    }

                    break;
                }
            }
        }

        Ok(ValidationResult {
            is_valid: violations.is_empty(),
            violations,
            files_processed: git_diff.changed_files.len(),
            files_matched,
        })
    }

    fn validate_file_against_rule(
        &self,
        file_path: &str,
        file_change: &crate::git::FileChange,
        rule: &Rule,
        verbose: bool,
    ) -> Result<Option<Violation>> {
        // For new files, we allow any content that matches the rule
        if file_change.change_type == ChangeType::Added {
            if verbose {
                println!("  New file - allowing all content");
            }

            return Ok(None);
        }

        // For deleted files, this is always a violation since they matched a rule
        if file_change.change_type == ChangeType::Deleted {
            if verbose {
                println!("  File deletion - violation (matches rule)");
            }
            return Ok(Some(Violation {
                file_path: file_path.to_string(),
                rule_description: format!(
                    "Rule matching {:?} prohibits deletion of files",
                    rule.match_conditions
                        .iter()
                        .map(|c| format!("{}={}", c.path, c.value))
                        .collect::<Vec<_>>()
                ),
                unauthorized_changes: vec!["File deletion".to_string()],
            }));
        }

        // Parse base content for modified files
        let base_content = match &file_change.base_content {
            Some(content) => content,
            None => {
                if verbose {
                    println!("  No base content available - allowing changes");
                }
                return Ok(None);
            }
        };

        let base_json = Self::parse_yaml_or_json(base_content)
            .with_context(|| format!("Failed to parse base content for {}", file_path))?;

        // Get current content for comparison
        let current_json = match &file_change.current_content {
            Some(content) => Self::parse_yaml_or_json(content)
                .with_context(|| format!("Failed to parse current content for {}", file_path))?,
            None => {
                return Err(anyhow::anyhow!(
                    "No current content available for modified file: {}",
                    file_path
                ));
            }
        };

        // Check if changes are allowed
        let changes_allowed = JsonPathMatcher::has_allowed_changes_only(
            &base_json,
            &current_json,
            &rule.allowed_changes,
            rule.when_conditions.as_deref(),
        )
        .with_context(|| format!("Failed to validate changes for {}", file_path))?;

        if !changes_allowed {
            if verbose {
                println!("  Found unauthorized changes");
            }

            let unauthorized_changes = self.find_unauthorized_changes(
                &base_json,
                &current_json,
                &rule.allowed_changes,
                rule.when_conditions.as_deref(),
            )?;

            return Ok(Some(Violation {
                file_path: file_path.to_string(),
                rule_description: format!(
                    "Rule matching {:?} allows only changes to: {:?}",
                    rule.match_conditions
                        .iter()
                        .map(|c| format!("{}={}", c.path, c.value))
                        .collect::<Vec<_>>(),
                    rule.allowed_changes
                ),
                unauthorized_changes,
            }));
        }

        if verbose {
            println!("  All changes are authorized");
        }
        Ok(None)
    }

    fn find_unauthorized_changes(
        &self,
        base_json: &Value,
        current_json: &Value,
        allowed_patterns: &[String],
        when_conditions: Option<&[crate::config::PathValue]>,
    ) -> Result<Vec<String>> {
        let all_changes = JsonPathMatcher::get_all_changes(base_json, current_json)?;
        let mut unauthorized = Vec::new();

        for change_path in all_changes.keys() {
            if !JsonPathMatcher::path_matches_any_pattern(change_path, allowed_patterns) {
                unauthorized.push(change_path.clone());

                continue;
            }

            if let Some(when_conditions) = when_conditions
                && !JsonPathMatcher::when_conditions_met(
                    current_json,
                    change_path,
                    when_conditions,
                )?
            {
                unauthorized.push(format!("{} (when condition not met)", change_path));
            }
        }

        Ok(unauthorized)
    }

    fn file_matches_rule(json: &Value, rule: &Rule) -> bool {
        JsonPathMatcher::matches_conditions(json, &rule.match_conditions)
    }

    fn parse_yaml_or_json(content: &str) -> Result<Value> {
        if let Ok(json) = serde_json::from_str(content) {
            return Ok(json);
        }

        serde_norway::from_str(content).context("Failed to parse as YAML or JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PathValue, Rule};
    use serde_json::json;

    #[test]
    fn test_file_matches_rule() {
        let json = json!({
            "kind": "ApplicationSet",
            "metadata": {
                "name": "test"
            }
        });

        let rule = Rule {
            match_conditions: vec![PathValue {
                path: "kind".to_string(),
                value: json!("ApplicationSet"),
            }],
            allowed_changes: vec![],
            when_conditions: None,
        };

        assert!(Validator::file_matches_rule(&json, &rule));
    }

    #[test]
    fn test_parse_yaml_content() {
        let yaml_content = r#"
kind: ApplicationSet
metadata:
  name: test
"#;
        let result = Validator::parse_yaml_or_json(yaml_content);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert_eq!(json["kind"], "ApplicationSet");
        assert_eq!(json["metadata"]["name"], "test");
    }

    #[test]
    fn test_parse_json_content() {
        let json_content = r#"{"kind": "ApplicationSet", "metadata": {"name": "test"}}"#;
        let result = Validator::parse_yaml_or_json(json_content);
        assert!(result.is_ok());

        let json = result.unwrap();
        assert_eq!(json["kind"], "ApplicationSet");
        assert_eq!(json["metadata"]["name"], "test");
    }
}
