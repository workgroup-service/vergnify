// @author Cursor

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::cache::{now_unix, VersionCache};
use crate::catalog::{self, LatestSource};
use crate::lines::{self, lines_from_endoflife, lines_from_go, lines_from_node, single_line};
use crate::model::ReleaseLine;
use crate::version::{to_semver, version_token};

pub async fn resolve_latest(
    ids: &[String],
    offline: bool,
    cache_path: &Path,
) -> (HashMap<String, Vec<ReleaseLine>>, Vec<String>) {
    let mut cache = VersionCache::load(cache_path);
    let now = now_unix();
    let mut resolved = HashMap::new();
    let mut warnings = Vec::new();
    let mut to_fetch = Vec::new();
    let mut missing_offline = false;

    for id in ids {
        if let Some(version) = cache.fresh(id, now) {
            resolved.insert(id.clone(), version);
        } else if offline {
            if let Some(version) = cache.any(id) {
                resolved.insert(id.clone(), version);
            } else {
                missing_offline = true;
            }
        } else {
            to_fetch.push(id.clone());
        }
    }

    if missing_offline {
        warnings.push(crate::i18n::offline_latest_unknown().into());
    }

    if to_fetch.is_empty() {
        return (resolved, warnings);
    }

    let client = match reqwest::Client::builder()
        .user_agent(concat!("vergnify/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            warnings.push(crate::i18n::http_client_failed(&error));
            return (resolved, warnings);
        }
    };

    let mut joins = tokio::task::JoinSet::new();
    for id in to_fetch {
        let client = client.clone();
        joins.spawn(async move {
            let result = fetch_one(&client, &id).await;
            (id, result)
        });
    }

    let mut changed = false;
    while let Some(joined) = joins.join_next().await {
        let Ok((id, result)) = joined else {
            warnings.push(crate::i18n::latest_task_failed().into());
            continue;
        };
        match result {
            Ok(version) => {
                cache.insert(id.clone(), version.clone(), now);
                resolved.insert(id, version);
                changed = true;
            }
            Err(error) => {
                if let Some(version) = cache.any(&id) {
                    resolved.insert(id.clone(), version);
                    warnings.push(crate::i18n::latest_used_cache(&id, &error));
                } else {
                    warnings.push(crate::i18n::latest_fetch_failed(&id, &error));
                }
            }
        }
    }

    if changed {
        if let Err(error) = cache.save(cache_path) {
            warnings.push(crate::i18n::latest_cache_save_failed(&error));
        }
    }

    (resolved, warnings)
}

async fn fetch_one(client: &reqwest::Client, id: &str) -> Result<Vec<ReleaseLine>, String> {
    let spec = catalog::find(id).ok_or_else(|| format!("unknown tool {id}"))?;
    let today = lines::today_iso(now_unix());
    match spec.latest {
        LatestSource::NodeLines => fetch_node(client, &today).await,
        LatestSource::GoLines => fetch_go(client).await,
        LatestSource::RustChannel => fetch_rust(client).await,
        LatestSource::EndOfLife { product, .. } => fetch_endoflife(client, product, &today).await,
        LatestSource::GitHub { owner, repo } => fetch_github(client, owner, repo).await,
        LatestSource::FlutterStable => fetch_flutter(client).await,
    }
}

async fn fetch_node(client: &reqwest::Client, today: &str) -> Result<Vec<ReleaseLine>, String> {
    let index_url = "https://nodejs.org/dist/index.json";
    let eol_url = "https://endoflife.date/api/nodejs.json";
    let (index, endoflife) = tokio::join!(get_text(client, index_url), get_text(client, eol_url));
    let index = index?;
    let endoflife = endoflife.ok();
    lines_from_node(&index, endoflife.as_deref(), today)
        .ok_or_else(|| "node releases have no stable version".into())
}

async fn fetch_go(client: &reqwest::Client) -> Result<Vec<ReleaseLine>, String> {
    let body = get_text(client, "https://go.dev/dl/?mode=json").await?;
    lines_from_go(&body).ok_or_else(|| "go download list has no stable version".into())
}

async fn fetch_rust(client: &reqwest::Client) -> Result<Vec<ReleaseLine>, String> {
    let body = get_text(
        client,
        "https://static.rust-lang.org/dist/channel-rust-stable.toml",
    )
    .await?;
    latest_from_rust_channel(&body)
        .map(|latest| single_line(&latest))
        .ok_or_else(|| "rust channel manifest has no version".into())
}

async fn fetch_endoflife(
    client: &reqwest::Client,
    product: &str,
    today: &str,
) -> Result<Vec<ReleaseLine>, String> {
    let url = format!("https://endoflife.date/api/{product}.json");
    let body = get_text(client, &url).await?;
    lines_from_endoflife(&body, today)
        .ok_or_else(|| format!("endoflife.date has no version for {product}"))
}

async fn fetch_flutter(client: &reqwest::Client) -> Result<Vec<ReleaseLine>, String> {
    let body = get_text(
        client,
        "https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json",
    )
    .await?;
    latest_from_flutter_releases(&body)
        .map(|latest| single_line(&latest))
        .ok_or_else(|| "Flutter releases have no stable version".into())
}

async fn fetch_github(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
) -> Result<Vec<ReleaseLine>, String> {
    let release_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let response = client
        .get(release_url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        let body = response.text().await.map_err(|error| error.to_string())?;
        if let Some(version) = latest_from_github_release(&body) {
            return Ok(single_line(&version));
        }
    }

    let tags_url = format!("https://api.github.com/repos/{owner}/{repo}/tags?per_page=100");
    let body = get_text(client, &tags_url).await?;
    latest_from_github_tags(&body)
        .map(|version| single_line(&version))
        .ok_or_else(|| "GitHub tags have no stable version".into())
}

async fn get_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    response.text().await.map_err(|error| error.to_string())
}

pub fn latest_from_rust_channel(body: &str) -> Option<String> {
    let mut in_rust_pkg = false;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_rust_pkg = line == "[pkg.rust]";
            continue;
        }
        if in_rust_pkg {
            if let Some(value) = line.strip_prefix("version") {
                let value = value
                    .trim()
                    .trim_start_matches('=')
                    .trim()
                    .trim_matches('"');
                return version_token(value);
            }
        }
    }
    None
}

pub fn latest_from_flutter_releases(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let stable = value.get("current_release")?.get("stable")?.as_str()?;
    let releases = value.get("releases")?.as_array()?;
    let version = releases.iter().find_map(|release| {
        let hash = release.get("hash")?.as_str()?;
        let channel = release.get("channel")?.as_str()?;
        if hash == stable && channel == "stable" {
            release.get("version")?.as_str()
        } else {
            None
        }
    })?;
    version_token(version)
}

pub fn latest_from_github_release(body: &str) -> Option<String> {
    let release: Value = serde_json::from_str(body).ok()?;
    let tag = release.get("tag_name")?.as_str()?;
    if is_prerelease_tag(tag) {
        return None;
    }
    version_token(tag)
}

pub fn latest_from_github_tags(body: &str) -> Option<String> {
    let tags: Vec<Value> = serde_json::from_str(body).ok()?;
    let mut best: Option<String> = None;
    for tag in tags {
        let Some(name) = tag.get("name").and_then(Value::as_str) else {
            continue;
        };
        if is_prerelease_tag(name) {
            continue;
        }
        let Some(token) = version_token(name) else {
            continue;
        };
        let replace = match &best {
            None => true,
            Some(current) => newer_than(&token, current),
        };
        if replace {
            best = Some(token);
        }
    }
    best
}

fn is_prerelease_tag(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["-rc", "-alpha", "-beta", "-pre"]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn newer_than(candidate: &str, current: &str) -> bool {
    match (to_semver(candidate), to_semver(current)) {
        (Some(candidate), Some(current)) => candidate.cmp(&current) == Ordering::Greater,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upstream_payloads() {
        assert_eq!(
            latest_from_rust_channel("[pkg.rust]\nversion = \"1.90.0 (abc 2025-09-14)\"\n"),
            Some("1.90.0".into())
        );
        assert_eq!(
            latest_from_github_release(r#"{"tag_name":"v2.47.0"}"#),
            Some("2.47.0".into())
        );
        assert_eq!(
            latest_from_github_tags(
                r#"[{"name":"v2.56.0-rc2"},{"name":"v2.56.0"},{"name":"v2.55.0"}]"#
            ),
            Some("2.56.0".into())
        );
        assert_eq!(
            latest_from_flutter_releases(
                r#"{"current_release":{"stable":"abc"},"releases":[{"hash":"def","version":"3.48.0-0.1.pre","channel":"beta"},{"hash":"abc","version":"3.47.8","channel":"stable"}]}"#
            ),
            Some("3.47.8".into())
        );
    }
}
