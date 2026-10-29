// @author Cursor

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cache::{self, TTL_SECS};
use crate::model::{Severity, ToolItem, Vulnerability};

const QUERY_URL: &str = "https://api.osv.dev/v1/query";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OsvPackage {
    pub ecosystem: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct OsvCache {
    #[serde(default)]
    entries: HashMap<String, OsvCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct OsvCacheEntry {
    vulnerabilities: Vec<Vulnerability>,
    fetched_at: u64,
}

#[derive(Serialize)]
struct QueryBody<'a> {
    version: &'a str,
    package: PackageBody<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<&'a str>,
}

#[derive(Serialize)]
struct PackageBody<'a> {
    name: &'a str,
    ecosystem: &'a str,
}

/// インストール済みバージョンを OSV.dev に照合する。
///
/// Python、Node.js、Deno、Bun、.NET、Elixir、Swift、Flutter、Docker、Podman、Git、
/// GitHub CLI、Terraform、kubectl、Helm、AWS CLI 2 系、Azure CLI、uv は
/// GIT エコシステムの上流リポジトリを使う。Go は公式の `stdlib`。
/// PHP、Ruby、Java の CVE は GIT レコードにバージョン範囲が無いため Bitnami を使う。
/// AWS CLI 1 系は PyPI の `awscli`。
pub async fn resolve(
    tools: &[ToolItem],
    offline: bool,
    cache_path: &Path,
) -> (HashMap<String, Vec<Vulnerability>>, Vec<String>) {
    let mut cache = OsvCache::load(cache_path);
    let now = cache::now_unix();
    let mut resolved = HashMap::new();
    let mut warnings = Vec::new();
    let mut to_fetch = Vec::new();

    if offline {
        warnings.push(crate::i18n::offline_vulns_cache_only().into());
    }

    for tool in tools {
        if tool.current_version.is_empty() || package_for(tool).is_none() {
            continue;
        }
        let key = cache_key(&tool.id, &tool.current_version);
        if let Some(vulns) = cache.fresh(&key, now) {
            resolved.insert(tool.id.clone(), vulns);
        } else if offline {
            if let Some(vulns) = cache.any(&key) {
                resolved.insert(tool.id.clone(), vulns);
            }
        } else {
            to_fetch.push(tool.clone());
        }
    }

    if to_fetch.is_empty() {
        return (resolved, warnings);
    }

    let client = match reqwest::Client::builder()
        .user_agent(concat!("vergnify/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            warnings.push(crate::i18n::osv_client_failed(&error));
            return (resolved, warnings);
        }
    };

    let mut joins = tokio::task::JoinSet::new();
    for tool in to_fetch {
        let client = client.clone();
        joins.spawn(async move {
            let result = fetch_tool(&client, &tool).await;
            (tool.id, tool.name, tool.current_version, result)
        });
    }

    let mut changed = false;
    while let Some(joined) = joins.join_next().await {
        let Ok((id, name, version, result)) = joined else {
            warnings.push(crate::i18n::vuln_task_failed().into());
            continue;
        };
        match result {
            Ok(vulns) => {
                cache.insert(cache_key(&id, &version), vulns.clone(), now);
                resolved.insert(id, vulns);
                changed = true;
            }
            Err(error) => {
                warnings.push(crate::i18n::vuln_fetch_failed(&name, &error));
            }
        }
    }

    if changed {
        if let Err(error) = cache.save(cache_path) {
            warnings.push(crate::i18n::vuln_cache_save_failed(&error));
        }
    }

    (resolved, warnings)
}

pub(crate) fn package_for(tool: &ToolItem) -> Option<OsvPackage> {
    package_for_id(&tool.id, &tool.current_version)
}

pub(crate) fn package_for_id(id: &str, version: &str) -> Option<OsvPackage> {
    Some(match id {
        "python" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/python/cpython",
        },
        "node" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/nodejs/node",
        },
        "deno" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/denoland/deno",
        },
        "bun" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/oven-sh/bun",
        },
        "php" => OsvPackage {
            ecosystem: "Bitnami",
            name: "php",
        },
        "go" => OsvPackage {
            ecosystem: "Go",
            name: "stdlib",
        },
        "rust" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/rust-lang/rust",
        },
        "ruby" => OsvPackage {
            ecosystem: "Bitnami",
            name: "ruby",
        },
        "elixir" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/elixir-lang/elixir",
        },
        "java" => OsvPackage {
            ecosystem: "Bitnami",
            name: "java",
        },
        "dotnet" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/dotnet/runtime",
        },
        "swift" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/swiftlang/swift",
        },
        "flutter" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/flutter/flutter",
        },
        "docker" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/moby/moby",
        },
        "podman" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/containers/podman",
        },
        "git" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/git/git",
        },
        "gh" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/cli/cli",
        },
        "terraform" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/hashicorp/terraform",
        },
        "kubectl" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/kubernetes/kubernetes",
        },
        "helm" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/helm/helm",
        },
        "aws" if version.starts_with("1.") => OsvPackage {
            ecosystem: "PyPI",
            name: "awscli",
        },
        "aws" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/aws/aws-cli",
        },
        "az" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/Azure/azure-cli",
        },
        "uv" => OsvPackage {
            ecosystem: "GIT",
            name: "https://github.com/astral-sh/uv",
        },
        _ => return None,
    })
}

pub(crate) fn vulnerabilities_from_value(value: &Value) -> Vec<Vulnerability> {
    let Some(vulns) = value.get("vulns").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for vuln in vulns {
        let item = vulnerability_from(vuln);
        if seen.insert(item.cve_id.clone()) {
            found.push(item);
        }
    }
    found.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.cve_id.cmp(&right.cve_id))
    });
    found
}

fn vulnerability_from(value: &Value) -> Vulnerability {
    let id = value.get("id").and_then(Value::as_str).unwrap_or("").trim();
    let aliases = value
        .get("aliases")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let cve_id = cve_id(id, &aliases);
    let title = title_from(value, &cve_id);
    let severity = severity_from(value);
    Vulnerability {
        cve_id,
        severity,
        title,
    }
}

fn cve_id(id: &str, aliases: &[&str]) -> String {
    if id.starts_with("CVE-") {
        return id.to_string();
    }
    if let Some(cve) = aliases.iter().find(|alias| alias.starts_with("CVE-")) {
        return (*cve).to_string();
    }
    if !id.is_empty() {
        return id.to_string();
    }
    aliases.first().copied().unwrap_or("unknown").to_string()
}

fn title_from(value: &Value, fallback: &str) -> String {
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(summary) = summary {
        return clip(summary);
    }
    let details = value
        .get("details")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    details.map(clip).unwrap_or_else(|| fallback.to_string())
}

fn clip(text: &str) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if ch == '\n' || index == 140 {
            if index == 140 {
                out.push('…');
            }
            break;
        }
        out.push(ch);
    }
    out
}

pub(crate) fn severity_from(value: &Value) -> Severity {
    let mut best: Option<f64> = None;
    if let Some(entries) = value.get("severity").and_then(Value::as_array) {
        for entry in entries {
            let Some(score) = entry.get("score").and_then(Value::as_str) else {
                continue;
            };
            if let Some(parsed) = score_text(score) {
                best = Some(best.map_or(parsed, |current| current.max(parsed)));
            }
        }
    }
    if let Some(score) = best {
        return severity_from_score(score);
    }
    label_severity(value).unwrap_or(Severity::Low)
}

pub(crate) fn severity_from_score(score: f64) -> Severity {
    if score >= 9.0 {
        Severity::Critical
    } else if score >= 7.0 {
        Severity::High
    } else if score >= 4.0 {
        Severity::Medium
    } else {
        Severity::Low
    }
}

pub(crate) fn score_text(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(parsed) = text.parse::<cvss::Cvss>() {
        return Some(parsed.score());
    }
    text.parse::<f64>()
        .ok()
        .filter(|score| (0.0..=10.0).contains(score))
}

fn label_severity(value: &Value) -> Option<Severity> {
    for key in ["database_specific", "ecosystem_specific"] {
        if let Some(label) = value
            .get(key)
            .and_then(|item| item.get("severity"))
            .and_then(Value::as_str)
        {
            if let Some(severity) = severity_from_label(label) {
                return Some(severity);
            }
        }
    }
    None
}

fn severity_from_label(label: &str) -> Option<Severity> {
    match label.trim().to_ascii_lowercase().as_str() {
        "critical" => Some(Severity::Critical),
        "high" => Some(Severity::High),
        "medium" | "moderate" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        _ => None,
    }
}

async fn fetch_tool(
    client: &reqwest::Client,
    tool: &ToolItem,
) -> Result<Vec<Vulnerability>, String> {
    let package =
        package_for(tool).ok_or_else(|| crate::i18n::osv_package_missing().to_string())?;
    let mut page_token = None;
    let mut combined = Vec::new();
    for _ in 0..5 {
        let body = QueryBody {
            version: &tool.current_version,
            package: PackageBody {
                name: package.name,
                ecosystem: package.ecosystem,
            },
            page_token: page_token.as_deref(),
        };
        let response = client
            .post(QUERY_URL)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let payload: Value = response.json().await.map_err(|error| error.to_string())?;
        combined.extend(vulnerabilities_from_value(&payload));
        page_token = payload
            .get("next_page_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        if page_token.is_none() {
            break;
        }
    }
    let mut seen = HashSet::new();
    combined.retain(|item| seen.insert(item.cve_id.clone()));
    combined.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.cve_id.cmp(&right.cve_id))
    });
    Ok(combined)
}

fn cache_key(id: &str, version: &str) -> String {
    format!("{id}@{version}")
}

impl OsvCache {
    fn load(path: &Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(&tmp, path).map_err(|error| error.to_string())
    }

    fn fresh(&self, key: &str, now: u64) -> Option<Vec<Vulnerability>> {
        let entry = self.entries.get(key)?;
        (now.saturating_sub(entry.fetched_at) < TTL_SECS).then(|| entry.vulnerabilities.clone())
    }

    fn any(&self, key: &str) -> Option<Vec<Vulnerability>> {
        self.entries
            .get(key)
            .map(|entry| entry.vulnerabilities.clone())
    }

    fn insert(&mut self, key: String, vulnerabilities: Vec<Vulnerability>, now: u64) {
        self.entries.insert(
            key,
            OsvCacheEntry {
                vulnerabilities,
                fetched_at: now,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tools_to_osv_packages() {
        let python = package_for_id("python", "3.9.6").unwrap();
        assert_eq!(python.ecosystem, "GIT");
        assert_eq!(python.name, "https://github.com/python/cpython");
        assert_eq!(package_for_id("go", "1.25.0").unwrap().name, "stdlib");
        assert_eq!(package_for_id("aws", "1.32.0").unwrap().ecosystem, "PyPI");
        assert_eq!(package_for_id("aws", "2.15.0").unwrap().ecosystem, "GIT");
        assert!(package_for_id("unknown", "1.0.0").is_none());
    }

    #[test]
    fn parses_cve_alias_and_cvss() {
        let payload = serde_json::json!({
            "vulns": [{
                "id": "GO-2023-2041",
                "aliases": ["CVE-2023-39318"],
                "summary": "HTML comments",
                "severity": [{
                    "type": "CVSS_V3",
                    "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"
                }]
            }]
        });
        let vulns = vulnerabilities_from_value(&payload);
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].cve_id, "CVE-2023-39318");
        assert_eq!(vulns[0].severity, Severity::Critical);
        assert_eq!(vulns[0].title, "HTML comments");
    }

    #[test]
    fn scores_known_cvss_vectors() {
        assert_eq!(
            score_text("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"),
            Some(9.8)
        );
        assert_eq!(
            score_text("CVSS:3.0/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N"),
            Some(7.5)
        );
        assert_eq!(
            score_text("CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:H/I:H/A:H"),
            Some(7.8)
        );
        assert_eq!(severity_from_score(6.9), Severity::Medium);
        assert_eq!(severity_from_score(7.0), Severity::High);
    }

    #[test]
    fn uses_qualitative_severity_when_there_is_no_vector() {
        let payload = serde_json::json!({
            "id": "GO-1",
            "summary": "stdlib issue",
            "database_specific": { "severity": "HIGH" }
        });
        assert_eq!(severity_from(&payload), Severity::High);
    }
}
