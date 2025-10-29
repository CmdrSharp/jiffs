use jiffs::json_path::JsonPathMatcher;
use serde_json::json;

#[test]
fn json_diff_detects_single_field_changes() {
    let base = json!({
        "apiVersion": "argoproj.io/v1alpha1",
        "kind": "ApplicationSet",
        "spec": {
            "generators": [
                {
                    "selector": {
                        "matchLabels": {
                            "env": "development"
                        }
                    },
                    "values": {
                        "revision": "main"
                    }
                }
            ]
        }
    });

    let current = json!({
        "apiVersion": "argoproj.io/v1alpha1",
        "kind": "ApplicationSet",
        "spec": {
            "generators": [
                {
                    "selector": {
                        "matchLabels": {
                            "env": "development"
                        }
                    },
                    "values": {
                        "revision": "feature-branch"
                    }
                }
            ]
        }
    });

    let changes = JsonPathMatcher::get_all_changes(&base, &current).unwrap();

    // Should detect the revision change
    println!("Changes detected: {:?}", changes);

    // Should have exactly one change at the revision path
    assert_eq!(changes.len(), 1);
    assert!(changes.contains_key("/spec/generators/0/values/revision"));

    let revision_change = &changes["/spec/generators/0/values/revision"];
    assert_eq!(revision_change.0, Some(json!("main")));
    assert_eq!(revision_change.1, Some(json!("feature-branch")));
}

#[test]
fn json_diff_handles_additions_and_modifications() {
    let base = json!({
        "spec": {
            "replicas": 3,
            "image": "nginx:1.20"
        }
    });

    let current = json!({
        "spec": {
            "replicas": 5,
            "image": "nginx:1.20",
            "newField": "added"
        }
    });

    let changes = JsonPathMatcher::get_all_changes(&base, &current).unwrap();

    println!("Changes: {:?}", changes);

    // Should detect both the replicas change and the new field addition
    assert!(changes.contains_key("/spec/replicas"));
    assert!(changes.contains_key("/spec/newField"));

    // Check the replicas change
    let replicas_change = &changes["/spec/replicas"];
    assert_eq!(replicas_change.0, Some(json!(3)));
    assert_eq!(replicas_change.1, Some(json!(5)));

    // Check the new field addition
    let new_field_change = &changes["/spec/newField"];
    assert_eq!(new_field_change.0, None);
    assert_eq!(new_field_change.1, Some(json!("added")));
}
