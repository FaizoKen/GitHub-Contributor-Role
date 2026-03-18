use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::condition::{Condition, ConditionField, ConditionOperator};

pub fn build_config_schema(conditions: &[Condition], verify_url: &str) -> Value {
    let c = conditions.first();

    let mut values = HashMap::new();

    if let Some(c) = c {
        values.insert("repo".to_string(), json!(c.repo));
        values.insert("field".to_string(), json!(c.field.json_key()));
        values.insert("operator".to_string(), json!(c.operator.key()));

        let value_key = format!("value_{}", c.field.json_key());
        let val = match &c.value {
            Value::Number(n) => json!(n),
            _ => json!(""),
        };
        values.insert(value_key, val);

        if c.operator == ConditionOperator::Between {
            if let Some(end) = &c.value_end {
                let end_key = format!("value_end_{}", c.field.json_key());
                let end_val = match end {
                    Value::Number(n) => json!(n),
                    _ => json!(""),
                };
                values.insert(end_key, end_val);
            }
        }
    }

    json!({
        "version": 1,
        "name": "GitHub Contributor Roles",
        "description": "Assign Discord roles based on contributions to a GitHub repository.",
        "sections": [
            {
                "title": "Getting Started",
                "fields": [
                    {
                        "type": "display",
                        "key": "info",
                        "label": "How it works",
                        "value": format!(
                            "This plugin assigns a Discord role to members who have contributed \
                             to a specific GitHub repository.\n\
                             \n\
                             Step 1 \u{2192} Members link their GitHub account at:\n\
                             {verify_url}\n\
                             \n\
                             Step 2 \u{2192} You configure a target repository and contribution \
                             requirement below.\n\
                             \n\
                             Step 3 \u{2192} Any verified member meeting the requirement gets this \
                             role automatically. Contributor data refreshes every 1\u{2013}4 hours."
                        )
                    }
                ]
            },
            {
                "title": "Target Repository",
                "description": "The GitHub repository to track contributions for.",
                "fields": [
                    {
                        "type": "text",
                        "key": "repo",
                        "label": "Repository",
                        "description": "Full name in owner/repo format (e.g. facebook/react, rust-lang/rust)",
                        "validation": {
                            "required": true,
                            "pattern": "^[a-zA-Z0-9._-]+/[a-zA-Z0-9._-]+$",
                            "pattern_message": "Use owner/repo format (e.g. facebook/react)"
                        }
                    }
                ]
            },
            {
                "title": "Contribution Requirement",
                "description": "Set the minimum contribution a member must have to earn this role.",
                "fields": [
                    {
                        "type": "select",
                        "key": "field",
                        "label": "Contribution type",
                        "description": "Which contribution metric to check.",
                        "validation": { "required": true },
                        "options": [
                            {"label": "Commits", "value": "commits"},
                            {"label": "Pull Requests (all)", "value": "pullRequests"},
                            {"label": "Merged Pull Requests", "value": "mergedPRs"},
                            {"label": "Issues Opened", "value": "issues"}
                        ]
                    },
                    {
                        "type": "select",
                        "key": "operator",
                        "label": "Comparison",
                        "default_value": "gte",
                        "options": [
                            {"label": "= equals", "value": "eq"},
                            {"label": "> greater than", "value": "gt"},
                            {"label": ">= at least", "value": "gte"},
                            {"label": "< less than", "value": "lt"},
                            {"label": "<= at most", "value": "lte"},
                            {"label": "\u{2194} between (range)", "value": "between"}
                        ]
                    },
                    {
                        "type": "number",
                        "key": "value_commits",
                        "label": "Commits",
                        "validation": { "required": true, "min": 0 },
                        "condition": { "field": "field", "equals": "commits" }
                    },
                    {
                        "type": "number",
                        "key": "value_end_commits",
                        "label": "Commits (end)",
                        "validation": { "min": 0 },
                        "pair_with": "value_commits",
                        "conditions": [
                            { "field": "field", "equals": "commits" },
                            { "field": "operator", "equals": "between" }
                        ]
                    },
                    {
                        "type": "number",
                        "key": "value_pullRequests",
                        "label": "Pull Requests",
                        "validation": { "required": true, "min": 0 },
                        "condition": { "field": "field", "equals": "pullRequests" }
                    },
                    {
                        "type": "number",
                        "key": "value_end_pullRequests",
                        "label": "Pull Requests (end)",
                        "validation": { "min": 0 },
                        "pair_with": "value_pullRequests",
                        "conditions": [
                            { "field": "field", "equals": "pullRequests" },
                            { "field": "operator", "equals": "between" }
                        ]
                    },
                    {
                        "type": "number",
                        "key": "value_mergedPRs",
                        "label": "Merged PRs",
                        "validation": { "required": true, "min": 0 },
                        "condition": { "field": "field", "equals": "mergedPRs" }
                    },
                    {
                        "type": "number",
                        "key": "value_end_mergedPRs",
                        "label": "Merged PRs (end)",
                        "validation": { "min": 0 },
                        "pair_with": "value_mergedPRs",
                        "conditions": [
                            { "field": "field", "equals": "mergedPRs" },
                            { "field": "operator", "equals": "between" }
                        ]
                    },
                    {
                        "type": "number",
                        "key": "value_issues",
                        "label": "Issues",
                        "validation": { "required": true, "min": 0 },
                        "condition": { "field": "field", "equals": "issues" }
                    },
                    {
                        "type": "number",
                        "key": "value_end_issues",
                        "label": "Issues (end)",
                        "validation": { "min": 0 },
                        "pair_with": "value_issues",
                        "conditions": [
                            { "field": "field", "equals": "issues" },
                            { "field": "operator", "equals": "between" }
                        ]
                    }
                ]
            },
            {
                "title": "Examples",
                "collapsible": true,
                "default_collapsed": true,
                "fields": [
                    {
                        "type": "display",
                        "key": "examples",
                        "label": "Common setups",
                        "value": "Commits >= 1  \u{2192}  Any contributor (at least one commit)\n\
                                  Commits >= 50  \u{2192}  Major contributors (50+ commits)\n\
                                  Merged PRs >= 1  \u{2192}  Has at least one merged pull request\n\
                                  Pull Requests >= 5  \u{2192}  Active PR contributor\n\
                                  Issues >= 10  \u{2192}  Frequent issue reporter\n\
                                  Commits between 1 to 10  \u{2192}  New contributors (1 to 10 commits)"
                    }
                ]
            }
        ],
        "values": values
    })
}

pub fn parse_config(config: &HashMap<String, Value>) -> Result<Vec<Condition>, AppError> {
    // Extract repo
    let repo = config
        .get("repo")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if repo.is_empty() {
        return Err(AppError::BadRequest("Repository is required".into()));
    }

    // Validate owner/repo format
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(AppError::BadRequest(
            "Repository must be in owner/repo format (e.g. facebook/react)".into(),
        ));
    }

    // Extract field
    let field_key = config
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if field_key.is_empty() {
        return Err(AppError::BadRequest("Contribution type is required".into()));
    }

    let field = ConditionField::from_key(field_key)
        .ok_or_else(|| AppError::BadRequest(format!("Invalid field '{field_key}'")))?;

    // Extract operator
    let op_key = config
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("gte");

    let operator = ConditionOperator::from_key(op_key)
        .ok_or_else(|| AppError::BadRequest(format!("Invalid operator '{op_key}'")))?;

    // Extract value
    let specific_key = format!("value_{field_key}");
    let raw_value = config.get(&specific_key).or_else(|| config.get("value"));

    let value_num = raw_value.and_then(|v| v.as_i64()).or_else(|| {
        raw_value
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
    });

    let value = value_num
        .map(|n| serde_json::Value::Number(n.into()))
        .ok_or_else(|| AppError::BadRequest(format!("Value is required for '{field_key}'")))?;

    if value_num.unwrap_or(0) < 0 {
        return Err(AppError::BadRequest("Value must be >= 0".into()));
    }

    // Parse end value for Between
    let value_end = if operator == ConditionOperator::Between {
        let end_specific_key = format!("value_end_{field_key}");
        let raw_end = config
            .get(&end_specific_key)
            .or_else(|| config.get("value_end"));

        let end_num = raw_end.and_then(|v| v.as_i64()).or_else(|| {
            raw_end
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
        });

        let end_val = end_num
            .map(|n| serde_json::Value::Number(n.into()))
            .ok_or_else(|| {
                AppError::BadRequest("End value is required for between operator".into())
            })?;

        // Validate start <= end
        if let (Some(start), Some(end)) = (value_num, end_num) {
            if start > end {
                return Err(AppError::BadRequest(
                    "Start value must be less than or equal to end value".into(),
                ));
            }
        }

        Some(end_val)
    } else {
        None
    };

    Ok(vec![Condition {
        repo,
        field,
        operator,
        value,
        value_end,
    }])
}
