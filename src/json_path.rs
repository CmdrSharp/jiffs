use anyhow::Result;
use json_patch::diff;
use jsonptr::Pointer;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

pub type ChangeMap = HashMap<String, (Option<Value>, Option<Value>)>;

pub struct JsonPathMatcher;

impl JsonPathMatcher {
    /// Check if a JSON document matches all the given path-value conditions
    pub fn matches_conditions(json: &Value, conditions: &[crate::config::PathValue]) -> bool {
        conditions
            .iter()
            .all(|condition| Self::matches_condition(json, &condition.path, &condition.value))
    }

    /// Check if a JSON document matches a single path-value condition (supports wildcards)
    pub fn matches_condition(json: &Value, path: &str, expected_value: &Value) -> bool {
        match Self::get_values_at_path(json, path) {
            Ok(values) => values.iter().any(|v| v == expected_value),
            Err(_) => false,
        }
    }

    /// Get all values at a given JSON path (supports wildcards) using JSON Pointer expansion
    pub fn get_values_at_path(json: &Value, path: &str) -> Result<Vec<Value>> {
        // Normalize path to always start with "/"
        let normalized_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        if normalized_path.contains('*') {
            Self::expand_wildcard_paths(json, &normalized_path)
        } else {
            match Self::get_value_at_json_pointer(json, &normalized_path) {
                Ok(value) => Ok(vec![value]),
                Err(_) => Ok(vec![]),
            }
        }
    }

    /// Expand wildcard paths by finding all matching array indices
    fn expand_wildcard_paths(json: &Value, wildcard_path: &str) -> Result<Vec<Value>> {
        let mut results = Vec::new();
        let path_parts: Vec<&str> = wildcard_path.split('/').filter(|s| !s.is_empty()).collect();

        Self::find_wildcard_matches(json, &path_parts, 0, "", &mut results)?;
        Ok(results)
    }

    /// Recursively find all paths that match the wildcard pattern
    fn find_wildcard_matches(
        current: &Value,
        path_parts: &[&str],
        part_index: usize,
        current_path: &str,
        results: &mut Vec<Value>,
    ) -> Result<()> {
        if part_index >= path_parts.len() {
            results.push(current.clone());

            return Ok(());
        }

        let part = path_parts[part_index];

        if part == "*" {
            match current {
                Value::Array(arr) => {
                    for (index, item) in arr.iter().enumerate() {
                        let new_path = format!("{}/{}", current_path, index);
                        Self::find_wildcard_matches(
                            item,
                            path_parts,
                            part_index + 1,
                            &new_path,
                            results,
                        )?;
                    }
                }
                _ => {
                    return Ok(());
                }
            }
        } else {
            let new_path = format!("{}/{}", current_path, part);

            match current {
                Value::Object(obj) => {
                    if let Some(next_value) = obj.get(part) {
                        Self::find_wildcard_matches(
                            next_value,
                            path_parts,
                            part_index + 1,
                            &new_path,
                            results,
                        )?;
                    }
                }
                Value::Array(arr) => {
                    if let Ok(index) = part.parse::<usize>()
                        && let Some(next_value) = arr.get(index)
                    {
                        Self::find_wildcard_matches(
                            next_value,
                            path_parts,
                            part_index + 1,
                            &new_path,
                            results,
                        )?;
                    }
                }
                _ => {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Check if any changes in the diff match the allowed change patterns
    pub fn has_allowed_changes_only(
        base_json: &Value,
        current_json: &Value,
        allowed_patterns: &[String],
        when_conditions: Option<&[crate::config::PathValue]>,
    ) -> Result<bool> {
        let changes = Self::get_all_changes(base_json, current_json)?;

        for change_path in changes.keys() {
            if !Self::path_matches_any_pattern(change_path, allowed_patterns) {
                return Ok(false);
            }

            let when_conditions_met = if let Some(when_conditions) = when_conditions {
                Self::when_conditions_met(current_json, change_path, when_conditions)?
            } else {
                true
            };

            if !when_conditions_met {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if when conditions are met for a specific change
    /// Uses the exact path from json-patch to resolve array indices
    pub fn when_conditions_met(
        json: &Value,
        change_path: &str,
        when_conditions: &[crate::config::PathValue],
    ) -> Result<bool> {
        // Extract array indices from the exact JSON Pointer path (e.g., "/spec/generators/0/values")
        let change_indices = Self::extract_indices_from_json_pointer(change_path);

        // For each when condition, check if it matches at the same array indices
        for when_condition in when_conditions {
            let when_path_resolved =
                Self::resolve_wildcard_path_with_indices(&when_condition.path, &change_indices);

            if !Self::check_condition_at_json_pointer(
                json,
                &when_path_resolved,
                &when_condition.value,
            )? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Extract array indices from a JSON Pointer path like "/spec/generators/0/values/revision"
    /// Returns a list of (segment, index) pairs for array access
    fn extract_indices_from_json_pointer(path: &str) -> Vec<(String, usize)> {
        let mut indices = Vec::new();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for i in 0..parts.len().saturating_sub(1) {
            if let Ok(index) = parts[i + 1].parse::<usize>() {
                indices.push((parts[i].to_string(), index));
            }
        }

        indices
    }

    /// Resolve a wildcard path using specific indices from a change path
    /// e.g., "/spec/generators/*/values" with indices from "/spec/generators/0/values/revision"
    /// becomes "/spec/generators/0/values"
    fn resolve_wildcard_path_with_indices(
        wildcard_path: &str,
        indices: &[(String, usize)],
    ) -> String {
        let mut result = wildcard_path.to_string();

        for (segment_name, index) in indices {
            let wildcard_pattern = format!("/{}/*", segment_name);
            let replacement = format!("/{}/{}", segment_name, index);
            result = result.replace(&wildcard_pattern, &replacement);
        }

        result
    }

    /// Check a condition directly using JSON Pointer (no wildcards)
    fn check_condition_at_json_pointer(
        json: &Value,
        json_pointer_path: &str,
        expected_value: &Value,
    ) -> Result<bool> {
        match Self::get_value_at_json_pointer(json, json_pointer_path) {
            Ok(actual_value) => Ok(actual_value == *expected_value),
            Err(_) => Ok(false), // Path doesn't exist, condition fails
        }
    }

    /// Get all changes between base and current JSON using json-patch
    pub fn get_all_changes(base_json: &Value, current_json: &Value) -> Result<ChangeMap> {
        let json_patch::Patch(operations) = diff(base_json, current_json);
        let mut changes = HashMap::new();

        // Each operation in the patch represents one atomic change
        for operation in operations {
            let path = operation.path().to_string();

            match operation {
                json_patch::PatchOperation::Add(add_op) => {
                    changes.insert(path, (None, Some(add_op.value)));
                }
                json_patch::PatchOperation::Remove(remove_op) => {
                    if let Ok(old_value) =
                        Self::get_value_at_json_pointer(base_json, &remove_op.path.to_string())
                    {
                        changes.insert(path, (Some(old_value), None));
                    }
                }
                json_patch::PatchOperation::Replace(replace_op) => {
                    if let Ok(old_value) =
                        Self::get_value_at_json_pointer(base_json, &replace_op.path.to_string())
                    {
                        changes.insert(path, (Some(old_value), Some(replace_op.value)));
                    }
                }
                _ => continue,
            }
        }

        Ok(changes)
    }

    /// Get a value at a specific JSON Pointer path using the standard jsonptr library
    fn get_value_at_json_pointer(json: &Value, pointer: &str) -> Result<Value> {
        let ptr = Pointer::parse(pointer)
            .map_err(|e| anyhow::anyhow!("Invalid JSON pointer '{}': {}", pointer, e))?;

        match ptr.resolve(json) {
            Ok(value) => Ok(value.clone()),
            Err(_) => Err(anyhow::anyhow!("Path '{}' not found in JSON", pointer)),
        }
    }

    /// Check if a path matches any of the allowed patterns
    pub fn path_matches_any_pattern(path: &str, patterns: &[String]) -> bool {
        patterns
            .iter()
            .any(|pattern| Self::path_matches_pattern(path, pattern))
    }

    /// Check if a path matches a pattern (supports wildcards)
    fn path_matches_pattern(path: &str, pattern: &str) -> bool {
        let regex_pattern = pattern.replace('*', r"\d+");

        let regex = match Regex::new(&format!("^{}$", regex_pattern)) {
            Ok(r) => r,
            Err(_) => return false,
        };

        regex.is_match(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_wildcard_expansion() {
        let json = json!({
            "spec": {
                "generators": [
                    {
                        "values": {
                            "revision": "main"
                        }
                    },
                    {
                        "values": {
                            "revision": "develop"
                        }
                    }
                ]
            }
        });

        let values =
            JsonPathMatcher::get_values_at_path(&json, "/spec/generators/*/values/revision")
                .unwrap();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&json!("main")));
        assert!(values.contains(&json!("develop")));
    }

    #[test]
    fn test_matches_condition() {
        let json = json!({
            "kind": "ApplicationSet",
            "spec": {
                "generators": [
                    {
                        "selector": {
                            "matchLabels": {
                                "env": "development"
                            }
                        }
                    }
                ]
            }
        });

        assert!(JsonPathMatcher::matches_condition(
            &json,
            "kind",
            &json!("ApplicationSet")
        ));
        assert!(JsonPathMatcher::matches_condition(
            &json,
            "/spec/generators/*/selector/matchLabels/env",
            &json!("development")
        ));
        assert!(!JsonPathMatcher::matches_condition(
            &json,
            "kind",
            &json!("Application")
        ));
    }

    #[test]
    fn test_path_matches_pattern() {
        assert!(JsonPathMatcher::path_matches_pattern(
            "/spec/generators/0/values/revision",
            "/spec/generators/*/values/revision"
        ));
        assert!(JsonPathMatcher::path_matches_pattern(
            "/spec/generators/1/values/revision",
            "/spec/generators/*/values/revision"
        ));
        assert!(!JsonPathMatcher::path_matches_pattern(
            "/spec/generators/0/values/other",
            "/spec/generators/*/values/revision"
        ));
    }
}
