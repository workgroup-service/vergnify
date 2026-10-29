// @author Cursor

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolItem {
    pub id: String,
    pub name: String,
    pub current_version: String,
    pub line: String,
    pub latest_version: String,
    pub newer: Vec<NewerLine>,
    pub binary_path: String,
    pub status: VersionStatus,
    pub vulnerabilities: Vec<Vulnerability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewerLine {
    pub label: String,
    pub latest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseLine {
    pub id: String,
    pub label: String,
    pub latest: String,
    pub support: LineSupport,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineSupport {
    ActiveLts,
    Maintenance,
    Current,
    Eol,
    Stable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    UpToDate,
    Outdated,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vulnerability {
    pub cve_id: String,
    pub severity: Severity,
    pub title: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// CVSS 7.0 以上は High か Critical になる。
    pub fn alerts(self) -> bool {
        matches!(self, Severity::High | Severity::Critical)
    }
}
