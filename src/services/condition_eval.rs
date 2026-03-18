use crate::models::condition::{Condition, ConditionField, ConditionOperator};
use crate::services::github::ContributorStats;

/// Evaluate a condition against contributor stats.
/// Pure sync function — no I/O, no allocations, microseconds.
pub fn evaluate_condition(condition: &Condition, stats: &ContributorStats) -> bool {
    let actual = match condition.field {
        ConditionField::Commits => stats.commits as i64,
        ConditionField::PullRequests => stats.pull_requests as i64,
        ConditionField::MergedPRs => stats.merged_prs as i64,
        ConditionField::Issues => stats.issues as i64,
    };

    let expected = condition.value.as_i64().unwrap_or(0);

    match condition.operator {
        ConditionOperator::Eq => actual == expected,
        ConditionOperator::Gt => actual > expected,
        ConditionOperator::Gte => actual >= expected,
        ConditionOperator::Lt => actual < expected,
        ConditionOperator::Lte => actual <= expected,
        ConditionOperator::Between => {
            let end = condition
                .value_end
                .as_ref()
                .and_then(|v| v.as_i64())
                .unwrap_or(expected);
            actual >= expected && actual <= end
        }
    }
}

/// Evaluate a condition when the user is NOT in the repo_contributors table.
/// If there are no stats, all metrics are 0.
pub fn evaluate_condition_zero(condition: &Condition) -> bool {
    let zero = ContributorStats {
        commits: 0,
        pull_requests: 0,
        merged_prs: 0,
        issues: 0,
    };
    evaluate_condition(condition, &zero)
}
