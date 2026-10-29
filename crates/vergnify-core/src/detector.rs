// @author Cursor

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::catalog::{self, ToolSpec};
use crate::version::{flutter_version, parse_version_output, swift_language_version};

const VERSION_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedTool {
    pub id: &'static str,
    pub name: &'static str,
    pub current_version: String,
    pub binary_path: String,
}

pub fn detect_installed() -> (Vec<DetectedTool>, Vec<String>) {
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    for spec in catalog::tools() {
        match detect_one(spec) {
            DetectOutcome::Found(tool) => found.push(tool),
            DetectOutcome::Missing => {}
            DetectOutcome::Warning(message) => warnings.push(message),
        }
    }
    (found, warnings)
}

fn version_of(id: &str, output: &str) -> Option<String> {
    match id {
        "swift" => swift_language_version(output),
        "flutter" => flutter_version(output),
        _ => parse_version_output(output),
    }
}

enum DetectOutcome {
    Found(DetectedTool),
    Missing,
    Warning(String),
}

fn detect_one(spec: &ToolSpec) -> DetectOutcome {
    let Some(path) = crate::locate::locate(spec.binaries) else {
        return DetectOutcome::Missing;
    };
    match run_version(&path, spec.args) {
        Ok(output) => match version_of(spec.id, &output) {
            Some(current_version) => DetectOutcome::Found(DetectedTool {
                id: spec.id,
                name: spec.name,
                current_version,
                binary_path: path.display().to_string(),
            }),
            None => {
                DetectOutcome::Warning(crate::i18n::version_unreadable(spec.name, &path.display()))
            }
        },
        Err(error) => DetectOutcome::Warning(crate::i18n::version_failed(spec.name, &error)),
    }
}

fn run_version(path: &Path, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| error.to_string())?;
                let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                text.push('\n');
                text.push_str(&String::from_utf8_lossy(&output.stderr));
                return Ok(text);
            }
            Ok(None) if started.elapsed() > VERSION_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timed out".into());
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(error.to_string()),
        }
    }
}
