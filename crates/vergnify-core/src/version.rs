// @author Cursor

use std::cmp::Ordering;

use semver::Version;

use crate::model::VersionStatus;

/// Returns the first `major.minor` or `major.minor.patch` token in `text`.
pub fn version_token(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let mut components = 1;
            let mut end = i;
            while components < 3 && i < chars.len() && chars[i] == '.' {
                let dot = i;
                i += 1;
                if i >= chars.len() || !chars[i].is_ascii_digit() {
                    i = dot;
                    break;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                components += 1;
                end = i;
            }
            if components >= 2 {
                return Some(chars[start..end].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    None
}

pub fn parse_version_output(output: &str) -> Option<String> {
    if let Some(version) = git_version_from_json(output) {
        return Some(version);
    }
    version_token(output)
}

/// Apple のツールチェーンは先に swift-driver の番号を出す。言語の版だけを取る。
pub fn swift_language_version(output: &str) -> Option<String> {
    for marker in ["Apple Swift version ", "Swift version "] {
        for line in output.lines() {
            if let Some((_, rest)) = line.split_once(marker) {
                return version_token(rest);
            }
        }
    }
    None
}

/// `Flutter x.y.z` の行だけを取る。同じ出力の Dart の番号は使わない。
pub fn flutter_version(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(rest) = line.trim().strip_prefix("Flutter ") {
            return version_token(rest);
        }
    }
    None
}

fn git_version_from_json(output: &str) -> Option<String> {
    let start = output.find('{')?;
    let value: serde_json::Value = serde_json::from_str(&output[start..]).ok()?;
    let git_version = value
        .get("clientVersion")
        .and_then(|client| client.get("gitVersion"))
        .and_then(|version| version.as_str())
        .or_else(|| value.get("gitVersion").and_then(|version| version.as_str()))?;
    version_token(git_version)
}

pub fn to_semver(raw: &str) -> Option<Version> {
    let token = version_token(raw)?;
    let mut parts = token.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next().unwrap_or("0");
    Version::parse(&format!("{major}.{minor}.{patch}")).ok()
}

pub fn judge(current: &str, latest: &str) -> VersionStatus {
    match (to_semver(current), to_semver(latest)) {
        (Some(current), Some(latest)) => {
            if current.cmp(&latest) != Ordering::Less {
                VersionStatus::UpToDate
            } else {
                VersionStatus::Outdated
            }
        }
        _ => VersionStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_versions_from_tool_output() {
        assert_eq!(version_token("v20.11.0"), Some("20.11.0".into()));
        assert_eq!(version_token("Python 3.12.1"), Some("3.12.1".into()));
        assert_eq!(
            version_token("go version go1.22.5 darwin/arm64"),
            Some("1.22.5".into())
        );
        assert_eq!(version_token("git version 2.43.0"), Some("2.43.0".into()));
        assert_eq!(
            version_token("java version \"21.0.2\" 2024-01-16 LTS"),
            Some("21.0.2".into())
        );
        assert_eq!(
            version_token("aws-cli/2.15.17 Python/3.11.6 Darwin/23.0.0"),
            Some("2.15.17".into())
        );
        assert_eq!(
            version_token("rustc 1.90.0 (1159e78c4 2025-09-14)"),
            Some("1.90.0".into())
        );
        assert_eq!(version_token("no version here"), None);
    }

    #[test]
    fn reads_the_swift_language_version() {
        let apple = "swift-driver version: 1.148.6 \nApple Swift version 6.3.2 (swiftlang-6.3.2.1.108 clang-2100.1.1.101)\n";
        assert_eq!(swift_language_version(apple), Some("6.3.2".into()));
        assert_eq!(
            swift_language_version("Swift version 6.0.3 (swift-6.0.3-RELEASE)\n"),
            Some("6.0.3".into())
        );
        assert_eq!(swift_language_version("swift-driver version: 1.148.6\n"), None);
    }

    #[test]
    fn reads_the_flutter_framework_version() {
        let output = "Flutter 3.47.8 • channel stable\nTools • Dart 3.5.4\n";
        assert_eq!(flutter_version(output), Some("3.47.8".into()));
        assert_eq!(
            flutter_version("Flutter 3.27.0-1.0.pre • channel beta\n"),
            Some("3.27.0".into())
        );
    }

    #[test]
    fn prefers_kubectl_git_version() {
        let output = r#"{"clientVersion":{"gitVersion":"v1.29.2"}} warning 9.9.9"#;
        assert_eq!(parse_version_output(output), Some("1.29.2".into()));
    }

    #[test]
    fn compares_numeric_components() {
        assert_eq!(judge("1.2.3", "1.2.3"), VersionStatus::UpToDate);
        assert_eq!(judge("1.2.10", "1.2.9"), VersionStatus::UpToDate);
        assert_eq!(judge("1.2.3", "1.2.4"), VersionStatus::Outdated);
        assert_eq!(judge("3.12", "3.12.0"), VersionStatus::UpToDate);
        assert_eq!(judge("3.12", "3.12.1"), VersionStatus::Outdated);
        assert_eq!(judge("1.2.3", ""), VersionStatus::Unknown);
        assert_eq!(judge("", "1.2.3"), VersionStatus::Unknown);
    }
}
