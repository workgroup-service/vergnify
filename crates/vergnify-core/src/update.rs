// @author Cursor

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::model::{ToolItem, VersionStatus};
use crate::scan::ScanReport;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateAction {
    pub tool_id: String,
    pub tool_name: String,
    pub manager: String,
    pub program: String,
    pub args: Vec<String>,
}

impl UpdateAction {
    pub fn command_line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    Planned,
    Updated,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateResult {
    pub tool_id: String,
    pub tool_name: String,
    pub manager: String,
    pub command: String,
    pub status: UpdateStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateReport {
    pub results: Vec<UpdateResult>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub missing_target: bool,
}

impl UpdateReport {
    pub fn failed(&self) -> bool {
        self.results
            .iter()
            .any(|result| result.status == UpdateStatus::Failed)
            || self.missing_target
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Host {
    pub brew_prefix: Option<PathBuf>,
    pub formulas: Vec<String>,
    pub casks: Vec<String>,
    pub apt: HashMap<String, String>,
    pub winget_list: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    Ready(UpdateAction),
    UnknownFormula,
    Unmanaged,
}

/// 管理下にあるツールの更新コマンドを返す。状態では絞らない。
pub fn actions_for(tools: &[ToolItem]) -> HashMap<String, UpdateAction> {
    let host = detect_host(tools);
    tools
        .iter()
        .filter_map(|tool| match decide(tool, &host) {
            Decision::Ready(action) => Some((tool.id.clone(), action)),
            Decision::UnknownFormula | Decision::Unmanaged => None,
        })
        .collect()
}

pub fn run_action(action: &UpdateAction) -> UpdateResult {
    let command = action.command_line();
    let output = Command::new(&action.program).args(&action.args).output();
    match output {
        Ok(output) => {
            let detail = output_detail(&output);
            let status = if output.status.success() {
                UpdateStatus::Updated
            } else {
                UpdateStatus::Failed
            };
            UpdateResult {
                tool_id: action.tool_id.clone(),
                tool_name: action.tool_name.clone(),
                manager: action.manager.clone(),
                command,
                status,
                detail,
            }
        }
        Err(error) => UpdateResult {
            tool_id: action.tool_id.clone(),
            tool_name: action.tool_name.clone(),
            manager: action.manager.clone(),
            command,
            status: UpdateStatus::Failed,
            detail: error.to_string(),
        },
    }
}

/// `target` が無いときは outdated と unsupported だけを更新する。
/// 名前を指定したときは、そのツールが管理下にあれば状態に関係なく更新する。
pub async fn update(target: Option<&str>, dry_run: bool) -> UpdateReport {
    let report = crate::check(false).await;
    plan_and_run(&report, target, dry_run)
}

pub(crate) fn plan_and_run(
    report: &ScanReport,
    target: Option<&str>,
    dry_run: bool,
) -> UpdateReport {
    let mut warnings = report.warnings.clone();
    let (selected, missing_target) = selected_tools(report, target, &mut warnings);
    let host = detect_host(&report.tools);
    let mut results = Vec::new();
    for tool in selected {
        match decide(tool, &host) {
            Decision::Ready(action) => {
                if dry_run {
                    results.push(UpdateResult {
                        tool_id: action.tool_id.clone(),
                        tool_name: action.tool_name.clone(),
                        manager: action.manager.clone(),
                        command: action.command_line(),
                        status: UpdateStatus::Planned,
                        detail: crate::i18n::dry_run_detail().into(),
                    });
                } else {
                    results.push(run_action(&action));
                }
            }
            Decision::UnknownFormula => {
                warnings.push(crate::i18n::brew_formula_missing(&tool.name))
            }
            Decision::Unmanaged => warnings.push(crate::i18n::unmanaged(&tool.name)),
        }
    }
    UpdateReport {
        results,
        warnings,
        missing_target,
    }
}

fn selected_tools<'a>(
    report: &'a ScanReport,
    target: Option<&str>,
    warnings: &mut Vec<String>,
) -> (Vec<&'a ToolItem>, bool) {
    match target {
        Some(name) => {
            let found = report
                .tools
                .iter()
                .filter(|tool| tool.id == name || tool.name.eq_ignore_ascii_case(name))
                .collect::<Vec<_>>();
            let missing = found.is_empty();
            if missing {
                warnings.insert(0, crate::i18n::target_missing(name));
            }
            (found, missing)
        }
        None => (
            report
                .tools
                .iter()
                .filter(|tool| {
                    matches!(
                        tool.status,
                        VersionStatus::Outdated | VersionStatus::Unsupported
                    )
                })
                .collect(),
            false,
        ),
    }
}

pub(crate) fn decide(tool: &ToolItem, host: &Host) -> Decision {
    let path = PathBuf::from(&tool.binary_path);
    if let Some(prefix) = &host.brew_prefix {
        if under_prefix(&path, prefix) {
            return brew_decision(tool, host, prefix);
        }
    }
    if let Some(app) = scoop_app(&tool.binary_path) {
        if !safe_package(&app) {
            return Decision::Unmanaged;
        }
        return Decision::Ready(action(tool, "scoop", "scoop", vec!["update".into(), app]));
    }
    if let Some(package) = host.apt.get(&tool.binary_path) {
        if safe_package(package) {
            return Decision::Ready(action(
                tool,
                "apt",
                "sudo",
                vec![
                    "-n".into(),
                    "apt-get".into(),
                    "install".into(),
                    "--only-upgrade".into(),
                    "-y".into(),
                    package.clone(),
                ],
            ));
        }
    }
    if let Some(id) = winget_match(tool, &host.winget_list) {
        return Decision::Ready(action(
            tool,
            "winget",
            "winget",
            vec![
                "upgrade".into(),
                "--id".into(),
                id,
                "--exact".into(),
                "--accept-package-agreements".into(),
                "--accept-source-agreements".into(),
                "--disable-interactivity".into(),
            ],
        ));
    }
    Decision::Unmanaged
}

fn brew_decision(tool: &ToolItem, host: &Host, prefix: &Path) -> Decision {
    let path = PathBuf::from(&tool.binary_path);
    if under_prefix(&path, &prefix.join("Caskroom")) {
        let Some(name) = first_installed(&cask_candidates(&tool.id), &host.casks) else {
            return Decision::UnknownFormula;
        };
        return Decision::Ready(action(
            tool,
            "homebrew",
            "brew",
            vec!["upgrade".into(), "--cask".into(), name],
        ));
    }
    let Some(name) = first_installed(
        &formula_candidates(&tool.id, &tool.current_version),
        &host.formulas,
    ) else {
        return Decision::UnknownFormula;
    };
    Decision::Ready(action(
        tool,
        "homebrew",
        "brew",
        vec!["upgrade".into(), name],
    ))
}

pub(crate) fn formula_candidates(id: &str, version: &str) -> Vec<String> {
    let major = component(version, 0);
    let minor = component(version, 1);
    match id {
        "python" => versioned(
            "python",
            major.as_deref(),
            minor.as_deref(),
            &["python3", "python"],
        ),
        "node" => major_only("node", major.as_deref(), &["node"]),
        "deno" => vec!["deno".into()],
        "bun" => vec!["bun".into()],
        "php" => versioned("php", major.as_deref(), minor.as_deref(), &["php"]),
        "ruby" => versioned("ruby", major.as_deref(), minor.as_deref(), &["ruby"]),
        "elixir" => vec!["elixir".into()],
        "java" => {
            let mut names = Vec::new();
            if let Some(major) = major {
                names.push(format!("temurin@{major}"));
                names.push(format!("openjdk@{major}"));
            }
            names.push("temurin".into());
            names.push("openjdk".into());
            names
        }
        "dotnet" => major_only("dotnet", major.as_deref(), &["dotnet"]),
        "swift" => vec!["swift".into()],
        "go" => vec!["go".into()],
        "rust" => vec!["rust".into()],
        "docker" => vec!["docker".into(), "docker-cli".into()],
        "podman" => vec!["podman".into()],
        "git" => vec!["git".into()],
        "gh" => vec!["gh".into()],
        "terraform" => vec!["terraform".into()],
        "kubectl" => vec!["kubernetes-cli".into()],
        "helm" => vec!["helm".into()],
        "aws" => vec!["awscli".into()],
        "az" => vec!["azure-cli".into()],
        "uv" => vec!["uv".into()],
        _ => Vec::new(),
    }
}

fn cask_candidates(id: &str) -> Vec<String> {
    match id {
        "docker" => vec!["docker".into(), "docker-desktop".into()],
        "dotnet" => vec!["dotnet-sdk".into(), "dotnet".into()],
        "flutter" => vec!["flutter".into()],
        other => formula_candidates(other, ""),
    }
}

fn versioned(
    name: &str,
    major: Option<&str>,
    minor: Option<&str>,
    fallbacks: &[&str],
) -> Vec<String> {
    let mut names = Vec::new();
    if let (Some(major), Some(minor)) = (major, minor) {
        names.push(format!("{name}@{major}.{minor}"));
    }
    names.extend(fallbacks.iter().map(|item| (*item).to_string()));
    names
}

fn major_only(name: &str, major: Option<&str>, fallbacks: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(major) = major {
        names.push(format!("{name}@{major}"));
    }
    names.extend(fallbacks.iter().map(|item| (*item).to_string()));
    names
}

fn component(version: &str, index: usize) -> Option<String> {
    version
        .split('.')
        .nth(index)
        .filter(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

fn first_installed(candidates: &[String], installed: &[String]) -> Option<String> {
    candidates
        .iter()
        .find(|candidate| {
            installed.iter().any(|item| item == *candidate) && safe_package(candidate)
        })
        .cloned()
}

pub(crate) fn scoop_app(path: &str) -> Option<String> {
    let parts = path.split(['/', '\\']).collect::<Vec<_>>();
    let index = parts.iter().position(|part| *part == "scoop")?;
    if parts.get(index + 1).copied() == Some("apps") {
        let app = parts.get(index + 2).copied()?;
        if app.is_empty() {
            return None;
        }
        return Some(app.to_string());
    }
    None
}

fn winget_match(tool: &ToolItem, listing: &str) -> Option<String> {
    if listing.trim().is_empty() {
        return None;
    }
    winget_candidates(&tool.id, &tool.current_version)
        .into_iter()
        .find(|id| winget_lists(listing, id) && safe_package(id))
}

pub(crate) fn winget_candidates(id: &str, version: &str) -> Vec<String> {
    let major = component(version, 0);
    let minor = component(version, 1);
    match id {
        "python" => {
            let mut names = Vec::new();
            if let (Some(major), Some(minor)) = (major.as_deref(), minor.as_deref()) {
                names.push(format!("Python.Python.{major}.{minor}"));
            }
            if let Some(major) = major {
                names.push(format!("Python.Python.{major}"));
            }
            names.push("Python.Python.3".into());
            names
        }
        "node" => vec!["OpenJS.NodeJS".into()],
        "deno" => vec!["DenoLand.Deno".into()],
        "bun" => vec!["Oven-sh.Bun".into()],
        "php" => vec!["PHP.PHP".into()],
        "go" => vec!["GoLang.Go".into()],
        "rust" => vec!["Rustlang.Rustup".into()],
        "ruby" => vec!["RubyInstallerTeam.Ruby".into()],
        "java" => {
            let mut names = Vec::new();
            if let Some(major) = major {
                names.push(format!("EclipseAdoptium.Temurin.{major}.JDK"));
            }
            names.push("EclipseAdoptium.Temurin.JDK".into());
            names
        }
        "dotnet" => {
            let mut names = Vec::new();
            if let Some(major) = major {
                names.push(format!("Microsoft.DotNet.SDK.{major}"));
            }
            names.push("Microsoft.DotNet.SDK".into());
            names
        }
        "swift" => vec!["Swift.Toolchain".into()],
        "docker" => vec!["Docker.DockerDesktop".into()],
        "podman" => vec!["RedHat.Podman".into()],
        "git" => vec!["Git.Git".into()],
        "gh" => vec!["GitHub.cli".into()],
        "terraform" => vec!["Hashicorp.Terraform".into()],
        "kubectl" => vec!["Kubernetes.kubectl".into()],
        "helm" => vec!["Helm.Helm".into()],
        "aws" => vec!["Amazon.AWSCLI".into()],
        "az" => vec!["Microsoft.AzureCLI".into()],
        "uv" => vec!["astral-sh.uv".into()],
        _ => Vec::new(),
    }
}

pub(crate) fn winget_lists(listing: &str, id: &str) -> bool {
    listing.lines().any(|line| {
        line.split_whitespace()
            .any(|column| column.eq_ignore_ascii_case(id))
    })
}

fn action(tool: &ToolItem, manager: &str, program: &str, args: Vec<String>) -> UpdateAction {
    UpdateAction {
        tool_id: tool.id.clone(),
        tool_name: tool.name.clone(),
        manager: manager.into(),
        program: program.into(),
        args,
    }
}

fn under_prefix(path: &Path, prefix: &Path) -> bool {
    if path.starts_with(prefix) {
        return true;
    }
    std::fs::canonicalize(path)
        .ok()
        .is_some_and(|canonical| canonical.starts_with(prefix))
}

fn safe_package(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '.' | '+' | '_' | '-'))
}

fn detect_host(tools: &[ToolItem]) -> Host {
    let brew_prefix = command_stdout("brew", &["--prefix"]).and_then(|text| {
        let path = PathBuf::from(text.trim());
        (!path.as_os_str().is_empty()).then_some(path)
    });
    let formulas = if brew_prefix.is_some() {
        command_lines("brew", &["list", "--formula"])
    } else {
        Vec::new()
    };
    let casks = if brew_prefix.is_some() {
        command_lines("brew", &["list", "--cask"])
    } else {
        Vec::new()
    };
    let mut apt = HashMap::new();
    if command_runs("dpkg") {
        for tool in tools {
            if let Some(package) = dpkg_owner(&tool.binary_path) {
                apt.insert(tool.binary_path.clone(), package);
            }
        }
    }
    let winget_list = if command_runs("winget") {
        command_stdout(
            "winget",
            &[
                "list",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        )
        .unwrap_or_default()
    } else {
        String::new()
    };
    Host {
        brew_prefix,
        formulas,
        casks,
        apt,
        winget_list,
    }
}

fn dpkg_owner(path: &str) -> Option<String> {
    let output = Command::new("dpkg").args(["-S", path]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    parse_dpkg(&text)
}

pub(crate) fn parse_dpkg(output: &str) -> Option<String> {
    let line = output.lines().next()?.trim();
    let name = line.split(':').next()?.trim();
    safe_package(name).then(|| name.to_string())
}

fn command_runs(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn command_lines(program: &str, args: &[&str]) -> Vec<String> {
    command_stdout(program, args)
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let line = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.is_empty() {
        if output.status.success() {
            crate::i18n::updated().into()
        } else {
            crate::i18n::exit_code(&output.status)
        }
    } else {
        let clipped = line.chars().take(180).collect::<String>();
        if output.status.success() {
            crate::i18n::updated_with_detail(&clipped)
        } else {
            clipped
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: &str, version: &str, path: &str) -> ToolItem {
        ToolItem {
            id: id.into(),
            name: id.into(),
            current_version: version.into(),
            line: String::new(),
            latest_version: String::new(),
            newer: Vec::new(),
            binary_path: path.into(),
            status: VersionStatus::Outdated,
            vulnerabilities: Vec::new(),
        }
    }

    #[test]
    fn brew_uses_the_versioned_formula_that_is_installed() {
        let host = Host {
            brew_prefix: Some(PathBuf::from("/opt/homebrew")),
            formulas: vec!["node@22".into(), "node".into()],
            ..Host::default()
        };
        let decision = decide(&tool("node", "22.14.0", "/opt/homebrew/bin/node"), &host);
        let Decision::Ready(action) = decision else {
            panic!("expected brew action");
        };
        assert_eq!(action.manager, "homebrew");
        assert_eq!(action.args, vec!["upgrade", "node@22"]);
    }

    #[test]
    fn system_git_is_not_a_homebrew_tool() {
        let host = Host {
            brew_prefix: Some(PathBuf::from("/opt/homebrew")),
            formulas: vec!["git".into()],
            ..Host::default()
        };
        assert_eq!(
            decide(&tool("git", "2.50.1", "/usr/bin/git"), &host),
            Decision::Unmanaged
        );
    }

    #[test]
    fn scoop_apt_and_winget_commands() {
        let scoop = decide(
            &tool(
                "git",
                "2.47.0",
                r"C:\Users\me\scoop\apps\git\current\bin\git.exe",
            ),
            &Host::default(),
        );
        let Decision::Ready(scoop) = scoop else {
            panic!("scoop");
        };
        assert_eq!(scoop.command_line(), "scoop update git");

        let mut apt = HashMap::new();
        apt.insert("/usr/bin/git".into(), "git".into());
        let apt = decide(
            &tool("git", "2.47.0", "/usr/bin/git"),
            &Host {
                apt,
                ..Host::default()
            },
        );
        let Decision::Ready(apt) = apt else {
            panic!("apt");
        };
        assert_eq!(
            apt.command_line(),
            "sudo -n apt-get install --only-upgrade -y git"
        );

        let winget = decide(
            &tool("node", "22.14.0", r"C:\Program Files\nodejs\node.exe"),
            &Host {
                winget_list: "OpenJS.NodeJS  22.14.0".into(),
                ..Host::default()
            },
        );
        let Decision::Ready(winget) = winget else {
            panic!("winget");
        };
        assert!(winget.command_line().contains("--id OpenJS.NodeJS"));
    }

    #[test]
    fn parses_dpkg_ownership() {
        assert_eq!(parse_dpkg("git: /usr/bin/git\n").as_deref(), Some("git"));
    }

    #[test]
    fn rejects_package_names_with_spaces() {
        assert!(!safe_package("git; rm -rf /"));
        assert!(safe_package("python@3.9"));
    }
}
