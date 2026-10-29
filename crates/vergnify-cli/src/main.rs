// @author Cursor

use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use vergnify_core::i18n;
use vergnify_core::{ScanReport, Severity, UpdateReport, UpdateStatus, VersionStatus};

#[derive(Debug, Parser)]
#[command(name = "vergnify", version)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List,
    Check {
        #[arg(long)]
        offline: bool,
    },
    Update {
        tool: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
}

fn localized_command() -> clap::Command {
    Cli::command()
        .about(i18n::cli_about())
        .after_help(i18n::cli_after_help())
        .mut_arg("json", |arg| arg.help(i18n::help_json()))
        .mut_subcommand("list", |sub| sub.about(i18n::help_list()))
        .mut_subcommand("check", |sub| {
            sub.about(i18n::help_check())
                .mut_arg("offline", |arg| arg.help(i18n::help_offline()))
        })
        .mut_subcommand("update", |sub| {
            sub.about(i18n::help_update())
                .mut_arg("tool", |arg| arg.help(i18n::help_tool()))
                .mut_arg("dry_run", |arg| arg.help(i18n::help_dry_run()))
        })
}

#[tokio::main]
async fn main() -> ExitCode {
    let matches = localized_command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let json = cli.json;
    let is_list = matches!(cli.command, Command::List);
    let report = match cli.command {
        Command::Update { tool, dry_run } => {
            let report = vergnify_core::update(tool.as_deref(), dry_run).await;
            return print_update(json, &report);
        }
        Command::List => vergnify_core::list(),
        Command::Check { offline } => vergnify_core::check(offline).await,
    };

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("{}", i18n::json_failed(&error));
                return ExitCode::from(1);
            }
        }
    } else {
        for warning in &report.warnings {
            eprintln!("warning: {warning}");
        }
        if is_list {
            print_list(&report);
        } else {
            print_check(&report);
        }
    }

    ExitCode::from(0)
}

fn print_list(report: &ScanReport) {
    if report.tools.is_empty() {
        println!("{}", i18n::no_tools());
        return;
    }
    let rows = report
        .tools
        .iter()
        .map(|tool| {
            vec![
                tool.name.clone(),
                tool.current_version.clone(),
                tool.binary_path.clone(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&["NAME", "VERSION", "PATH"], &rows);
}

fn print_check(report: &ScanReport) {
    if report.tools.is_empty() {
        println!("{}", i18n::no_tools());
        return;
    }
    let rows = report
        .tools
        .iter()
        .map(|tool| {
            vec![
                tool.name.clone(),
                display_version(&tool.current_version),
                display_version(&tool.line),
                display_version(&tool.latest_version),
                status_label(tool.status).to_string(),
                display_newer(&tool.newer),
                display_vulns(&tool.vulnerabilities),
            ]
        })
        .collect::<Vec<_>>();
    print_table(
        &[
            "NAME",
            "CURRENT",
            "LINE",
            "LATEST IN LINE",
            "STATUS",
            "NEWER",
            "CVE",
        ],
        &rows,
    );
}

fn print_update(json: bool, report: &UpdateReport) -> ExitCode {
    if json {
        match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("{}", i18n::json_failed(&error));
                return ExitCode::from(1);
            }
        }
    } else {
        for warning in &report.warnings {
            eprintln!("warning: {warning}");
        }
        if report.results.is_empty() {
            println!("{}", i18n::nothing_to_update());
        } else {
            for result in &report.results {
                println!(
                    "{}  {}  {}  {}",
                    result.tool_name,
                    result.manager,
                    result.command,
                    update_label(result.status)
                );
                if result.status != UpdateStatus::Planned {
                    println!("  {}", result.detail);
                }
            }
        }
    }
    if report.failed() {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn display_vulns(vulns: &[vergnify_core::Vulnerability]) -> String {
    if vulns.is_empty() {
        return "-".to_string();
    }
    let worst = vulns.iter().map(|vuln| vuln.severity).max().unwrap();
    let shown = vulns
        .iter()
        .take(3)
        .map(|vuln| vuln.cve_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if vulns.len() > 3 {
        format!("{shown} +{} ({})", vulns.len() - 3, severity_label(worst))
    } else {
        format!("{shown} ({})", severity_label(worst))
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    }
}

fn update_label(status: UpdateStatus) -> &'static str {
    match status {
        UpdateStatus::Planned => "planned",
        UpdateStatus::Updated => "updated",
        UpdateStatus::Failed => "failed",
    }
}

fn display_version(version: &str) -> String {
    if version.is_empty() {
        "-".to_string()
    } else {
        version.to_string()
    }
}

fn display_newer(newer: &[vergnify_core::NewerLine]) -> String {
    if newer.is_empty() {
        "-".to_string()
    } else {
        newer
            .iter()
            .map(|line| format!("{} ({})", line.label, line.latest))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn status_label(status: VersionStatus) -> &'static str {
    match status {
        VersionStatus::UpToDate => "up-to-date",
        VersionStatus::Outdated => "outdated",
        VersionStatus::Unsupported => "unsupported",
        VersionStatus::Unknown => "unknown",
    }
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }
    print_row(
        headers.iter().map(|header| (*header).to_string()).collect(),
        &widths,
    );
    for row in rows {
        print_row(row.clone(), &widths);
    }
}

fn print_row(cells: Vec<String>, widths: &[usize]) {
    let line = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| format!("{cell:<width$}", width = widths[index]))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{}", line.trim_end());
}
