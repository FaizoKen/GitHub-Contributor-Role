use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConditionField {
    Commits,
    PullRequests,
    MergedPRs,
    Issues,
}

impl ConditionField {
    pub fn json_key(&self) -> &'static str {
        match self {
            Self::Commits => "commits",
            Self::PullRequests => "pullRequests",
            Self::MergedPRs => "mergedPRs",
            Self::Issues => "issues",
        }
    }

    pub fn sql_column(&self) -> &'static str {
        match self {
            Self::Commits => "rc.commits",
            Self::PullRequests => "rc.pull_requests",
            Self::MergedPRs => "rc.merged_prs",
            Self::Issues => "rc.issues",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "commits" => Some(Self::Commits),
            "pullRequests" => Some(Self::PullRequests),
            "mergedPRs" => Some(Self::MergedPRs),
            "issues" => Some(Self::Issues),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConditionOperator {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
    Between,
}

impl ConditionOperator {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "eq" => Some(Self::Eq),
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            "between" => Some(Self::Between),
            _ => None,
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Between => "between",
        }
    }

    pub fn sql_operator(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Between => "BETWEEN",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub repo: String,
    pub field: ConditionField,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_end: Option<serde_json::Value>,
}
