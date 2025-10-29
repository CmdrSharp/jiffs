#[cfg(test)]
mod validation_rules {
    use anyhow::Result;
    use jiffs::config::PathValue;
    use jiffs::json_path::JsonPathMatcher;
    use serde_json::json;

    #[test]
    fn when_conditions_enforce_environment_restrictions() -> Result<()> {
        // Simulate ApplicationSet with multiple generators
        let base_json = json!({
            "spec": {
                "generators": [
                    {
                        "clusters": {
                            "selector": {
                                "matchLabels": {
                                    "env": "development"
                                }
                            },
                            "values": {
                                "revision": "0.19.2"
                            }
                        }
                    },
                    {
                        "clusters": {
                            "selector": {
                                "matchLabels": {
                                    "env": "production"
                                }
                            },
                            "values": {
                                "revision": "0.19.2"
                            }
                        }
                    }
                ]
            }
        });

        // Change only production environment
        let current_json = json!({
            "spec": {
                "generators": [
                    {
                        "clusters": {
                            "selector": {
                                "matchLabels": {
                                    "env": "development"
                                }
                            },
                            "values": {
                                "revision": "0.19.2"
                            }
                        }
                    },
                    {
                        "clusters": {
                            "selector": {
                                "matchLabels": {
                                    "env": "production"
                                }
                            },
                            "values": {
                                "revision": "0.20.0"
                            }
                        }
                    }
                ]
            }
        });

        let allowed_patterns = vec!["/spec/generators/*/clusters/values/revision".to_string()];

        // Test 1: When condition allows changes to development only
        let when_dev = vec![PathValue {
            path: "/spec/generators/*/clusters/selector/matchLabels/env".to_string(),
            value: json!("development"),
        }];

        let result_dev = JsonPathMatcher::has_allowed_changes_only(
            &base_json,
            &current_json,
            &allowed_patterns,
            Some(&when_dev),
        )?;

        // Should be false because we changed production, but when condition only allows development
        assert!(
            !result_dev,
            "Production change should be rejected when when condition is for development"
        );

        // Test 2: When condition allows changes to production only
        let when_prod = vec![PathValue {
            path: "/spec/generators/*/clusters/selector/matchLabels/env".to_string(),
            value: json!("production"),
        }];

        let result_prod = JsonPathMatcher::has_allowed_changes_only(
            &base_json,
            &current_json,
            &allowed_patterns,
            Some(&when_prod),
        )?;

        // Should be true because we changed production and when condition allows production
        assert!(
            result_prod,
            "Production change should be allowed when when condition matches production"
        );

        // Test 3: No when conditions (always allow if path matches)
        let result_no_when = JsonPathMatcher::has_allowed_changes_only(
            &base_json,
            &current_json,
            &allowed_patterns,
            None,
        )?;

        // Should be true because the path matches and there are no when conditions
        assert!(
            result_no_when,
            "Change should be allowed when no when conditions exist"
        );

        Ok(())
    }
}
