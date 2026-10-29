// @author Cursor

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cache::default_cache_path;
use crate::catalog::{self, LineKey};
use crate::detector::{self, DetectedTool};
use crate::lines::judge_lines;
use crate::model::{ToolItem, VersionStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanReport {
    pub tools: Vec<ToolItem>,
    pub warnings: Vec<String>,
}

pub fn list() -> ScanReport {
    let (detected, warnings) = detector::detect_installed();
    ScanReport {
        tools: detected.into_iter().map(installed_item).collect(),
        warnings,
    }
}

pub async fn check(offline: bool) -> ScanReport {
    check_with(offline, default_cache_path()).await
}

pub async fn check_with(offline: bool, cache_path: PathBuf) -> ScanReport {
    let (detected, mut warnings) = detector::detect_installed();
    let ids = detected
        .iter()
        .map(|tool| tool.id.to_string())
        .collect::<Vec<_>>();
    let (latest, mut fetch_warnings) =
        crate::registry::resolve_latest(&ids, offline, &cache_path).await;
    warnings.append(&mut fetch_warnings);

    let mut tools = detected
        .into_iter()
        .map(|tool| {
            let lines = latest.get(tool.id).cloned().unwrap_or_default();
            let key = catalog::find(tool.id)
                .map(|spec| spec.line_key)
                .unwrap_or(LineKey::Single);
            let judged = judge_lines(&tool.current_version, &lines, key);
            ToolItem {
                id: tool.id.to_string(),
                name: tool.name.to_string(),
                current_version: tool.current_version,
                line: judged.line,
                latest_version: judged.latest_version,
                newer: judged.newer,
                binary_path: tool.binary_path,
                status: judged.status,
                vulnerabilities: Vec::new(),
            }
        })
        .collect::<Vec<_>>();

    let osv_path = cache_path.with_file_name("osv.json");
    let (vulnerabilities, mut osv_warnings) = crate::osv::resolve(&tools, offline, &osv_path).await;
    warnings.append(&mut osv_warnings);
    for tool in &mut tools {
        if let Some(found) = vulnerabilities.get(&tool.id) {
            tool.vulnerabilities.clone_from(found);
        }
    }
    if let Err(error) = crate::notify::notify_alerts(&tools) {
        warnings.push(crate::i18n::notify_failed(&error));
    }

    ScanReport { tools, warnings }
}

fn installed_item(tool: DetectedTool) -> ToolItem {
    ToolItem {
        id: tool.id.to_string(),
        name: tool.name.to_string(),
        current_version: tool.current_version,
        line: String::new(),
        latest_version: String::new(),
        newer: Vec::new(),
        binary_path: tool.binary_path,
        status: VersionStatus::Unknown,
        vulnerabilities: Vec::new(),
    }
}
