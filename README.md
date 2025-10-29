# Jiffs - JSON Diff Validation

A CLI tool for validating changes in Git repositories against configurable policy rules. When combined with code ownership, Jiffs can enable selective changes to certain parts of files without a review process.

## Overview

Jiffs analyzes Git diffs for JSON/YAML files and validates changes against policy rules defined in a configuration file. This is particularly useful in CI/CD workflows where you want to allow developers to modify specific fields (like development environment configurations) without requiring code review, while still enforcing a review process for production changes.

- **Policy-driven validation**: Define rules using JSON pointers to specify which changes are allowed
- **Conditional logic**: Apply different rules based on file content conditions
- **Git integration**: Analyzes changes between Git commits or branches

## Usage

```
Validate git diff changes against policy rules

Usage: jiffs [OPTIONS] --base <BASE> --policy <POLICY>

Options:
      --base <BASE>                  Base SHA to diff against
      --policy <POLICY>              Path to policy YAML
      --only-suffix <ONLY_SUFFIXES>  Optional: limit to files matching this suffix (repeatable). Example: --only-suffix .yaml --only-suffix .yml
  -v, --verbose                      Optional: verbose output (prints all changed paths)
  -h, --help                         Print help
  -V, --version                      Print version
```

### Examples

```bash
# Basic validation against main branch
jiffs --base main --policy rules.yaml

# Use in GitHub Actions
jiffs --base ${{ github.event.pull_request.base.sha }} --policy .github/policy-rules.yaml
```

## Policy Configuration

Policy rules are defined in a YAML file with the following structure:

```yaml
rules:
  - match:              # Conditions that must be met to apply this rule
    - path: <json-pointer>
      value: <expected-value>
    allowedChanges:     # JSON pointers to paths that can be modified
    - <json-pointer>
    when:               # Only allow the changes when these conditions match
    - path: <json-pointer>
      value: <expected-value>
```

### JSON Pointers

Jiffs uses [JSON Pointer (RFC 6901)](https://tools.ietf.org/html/rfc6901) syntax to specify paths within JSON/YAML documents:

- `/spec/template/name` - Direct path
- `/spec/generators/*/clusters/values/revision` - Wildcard for array elements
- `/metadata/labels/env` - Nested object access

### Example: ArgoCD ApplicationSet

```yaml
rules:
  # Allow revision changes for development environments only
  - match:
    - path: /kind
      value: ApplicationSet
    allowedChanges:
    - /spec/generators/*/clusters/values/revision
    when:
    - path: /spec/generators/*/clusters/selector/matchLabels/env
      value: development
```

This rule:
1. Applies to files where `kind` equals "ApplicationSet"
2. Allows changes to the `revision` field in cluster generators
3. Only when the environment label for that _same_ generator index is set to "development"

## GitHub Actions

This is an example action. This assumes the jiffs binary exists in-repo.

```yaml
name: Policy Validation
on:
  pull_request:
    branches: [ main ]
    types: [opened, reopened, synchronize, ready_for_review]
permissions:
  contents: read
  pull-requests: read

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    
    - name: Validate Changes
      run: |
        jiffs --base ${{ github.event.pull_request.base.sha }} \
              --policy .github/jiffs-rules.yaml \
              --only-suffix .yaml --only-suffix .yml \
              --verbose
```
