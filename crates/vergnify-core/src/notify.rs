// @author Cursor

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache;
use crate::model::{Severity, ToolItem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alert {
    pub tool_name: String,
    pub cve_id: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Notified {
    #[serde(default)]
    ids: HashSet<String>,
}

pub fn notify_path() -> PathBuf {
    cache::cache_dir().join("notified.json")
}

/// CVSS 7.0 以上で、まだ通知していない脆弱性だけを OS 通知する。
pub fn notify_alerts(tools: &[ToolItem]) -> Result<(), String> {
    notify_alerts_at(tools, &notify_path())
}

pub fn notify_alerts_at(tools: &[ToolItem], path: &Path) -> Result<(), String> {
    let mut seen = Notified::load(path);
    let fresh = pending_alerts(tools, &seen.ids);
    if fresh.is_empty() {
        return Ok(());
    }
    let body = notification_body(&fresh);
    notify_rust::Notification::new()
        .summary("Vergnify")
        .body(&body)
        .show()
        .map_err(|error| error.to_string())?;
    for alert in fresh {
        seen.ids.insert(alert_key(&alert));
    }
    seen.save(path)
}

pub(crate) fn pending_alerts(tools: &[ToolItem], seen: &HashSet<String>) -> Vec<Alert> {
    let mut alerts = Vec::new();
    for tool in tools {
        for vuln in &tool.vulnerabilities {
            if !vuln.severity.alerts() {
                continue;
            }
            let alert = Alert {
                tool_name: tool.name.clone(),
                cve_id: vuln.cve_id.clone(),
                severity: vuln.severity,
            };
            if !seen.contains(&alert_key(&alert)) {
                alerts.push(alert);
            }
        }
    }
    alerts
}

pub(crate) fn notification_body(alerts: &[Alert]) -> String {
    let mut lines = Vec::new();
    for alert in alerts.iter().take(8) {
        lines.push(format!(
            "{}: {} ({})",
            alert.tool_name,
            alert.cve_id,
            crate::i18n::severity_word(alert.severity)
        ));
    }
    if alerts.len() > 8 {
        lines.push(crate::i18n::more_alerts(alerts.len() - 8));
    }
    lines.join("\n")
}

fn alert_key(alert: &Alert) -> String {
    format!("{}:{}", alert.tool_name, alert.cve_id)
}

impl Notified {
    fn load(path: &Path) -> Self {
        let Ok(bytes) = fs::read(path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        fs::write(path, bytes).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{VersionStatus, Vulnerability};

    fn sample_vuln(id: &str, severity: Severity) -> Vulnerability {
        Vulnerability {
            cve_id: id.into(),
            severity,
            title: id.into(),
        }
    }

    fn tool(name: &str, vulns: Vec<Vulnerability>) -> ToolItem {
        ToolItem {
            id: name.to_ascii_lowercase(),
            name: name.into(),
            current_version: "1.0.0".into(),
            line: String::new(),
            latest_version: String::new(),
            newer: Vec::new(),
            binary_path: String::new(),
            status: VersionStatus::Outdated,
            vulnerabilities: vulns,
        }
    }

    #[test]
    fn alerts_only_for_unseen_high_and_critical() {
        let tools = vec![tool(
            "Python",
            vec![
                sample_vuln("CVE-1", Severity::High),
                sample_vuln("CVE-2", Severity::Medium),
                sample_vuln("CVE-3", Severity::Critical),
            ],
        )];
        let mut seen = HashSet::new();
        seen.insert("Python:CVE-1".into());
        let pending = pending_alerts(&tools, &seen);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].cve_id, "CVE-3");
    }

    #[test]
    fn medium_does_not_alert() {
        assert!(!Severity::Medium.alerts());
        assert!(Severity::High.alerts());
        assert!(Severity::Critical.alerts());
    }
}
