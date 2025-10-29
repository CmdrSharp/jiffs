use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    #[serde(rename = "match")]
    pub match_conditions: Vec<PathValue>,
    #[serde(rename = "allowedChanges")]
    pub allowed_changes: Vec<String>,
    #[serde(rename = "when")]
    pub when_conditions: Option<Vec<PathValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValue {
    pub path: String,
    pub value: serde_json::Value,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Config =
            serde_norway::from_str(&content).with_context(|| "Failed to parse YAML config")?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_rules_yaml() {
        let yaml_content = r#"
rules:
  - match:
    - path: kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/values/revision
    when:
    - path: /spec/generators/*/selector/matchLabels/env
      value: development
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", yaml_content).unwrap();

        let config = Config::from_file(temp_file.path()).unwrap();

        assert_eq!(config.rules.len(), 1);

        let rule = &config.rules[0];
        assert_eq!(rule.match_conditions.len(), 1);
        assert_eq!(rule.match_conditions[0].path, "kind");
        assert_eq!(
            rule.match_conditions[0].value,
            serde_json::Value::String("ApplicationSet".to_string())
        );

        assert_eq!(rule.allowed_changes.len(), 1);
        assert_eq!(
            rule.allowed_changes[0],
            "/spec/generators/*/values/revision"
        );

        assert!(rule.when_conditions.is_some());
        let when_conditions = rule.when_conditions.as_ref().unwrap();
        assert_eq!(when_conditions.len(), 1);
        assert_eq!(
            when_conditions[0].path,
            "/spec/generators/*/selector/matchLabels/env"
        );
        assert_eq!(
            when_conditions[0].value,
            serde_json::Value::String("development".to_string())
        );
    }
}
