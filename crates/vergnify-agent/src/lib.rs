// @author Cursor

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vergnify_core::ToolItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Inventory {
    pub hostname: String,
    pub os: String,
    pub tools: Vec<ToolItem>,
}

pub async fn collect() -> Inventory {
    let report = vergnify_core::check(false).await;
    Inventory {
        hostname: machine_hostname(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        tools: report.tools,
    }
}

pub async fn send(admin_url: &str, token: &str, inventory: &Inventory) -> Result<(), String> {
    let url = format!("{}/api/inventory", admin_url.trim_end_matches('/'));
    let client = http_client()?;
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(inventory)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", response.status()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credential {
    pub admin_url: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct DeviceStart {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct DevicePoll {
    token: Option<String>,
    error: Option<String>,
}

pub struct Enrollment {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: Duration,
}

pub fn credential_path() -> PathBuf {
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config)
            .join("vergnify")
            .join("agent-credential.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            return home.join("Library/Application Support/vergnify/agent-credential.json");
        }
        return home.join(".config/vergnify/agent-credential.json");
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local)
            .join("vergnify")
            .join("agent-credential.json");
    }
    PathBuf::from("agent-credential.json")
}

pub fn load_credential() -> Option<Credential> {
    let text = std::fs::read_to_string(credential_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_credential(credential: &Credential) -> Result<(), String> {
    let path = credential_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_vec_pretty(credential).map_err(|error| error.to_string())?;
    std::fs::write(&path, body).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub async fn begin_enrollment(admin_url: &str) -> Result<Enrollment, String> {
    let client = http_client()?;
    let response = client
        .post(format!(
            "{}/api/device/code",
            admin_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "hostname": machine_hostname(),
            "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let started: DeviceStart = response.json().await.map_err(|error| error.to_string())?;
    Ok(Enrollment {
        user_code: started.user_code,
        verification_uri: started.verification_uri,
        device_code: started.device_code,
        interval: Duration::from_secs(started.interval.max(1)),
    })
}

pub async fn poll_enrollment(admin_url: &str, device_code: &str) -> Result<Option<String>, String> {
    let client = http_client()?;
    let response = client
        .post(format!(
            "{}/api/device/token",
            admin_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({ "device_code": device_code }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body: DevicePoll = response.json().await.map_err(|error| error.to_string())?;
    if let Some(token) = body.token.filter(|token| !token.is_empty()) {
        return Ok(Some(token));
    }
    match body.error.as_deref() {
        Some("authorization_pending") => Ok(None),
        Some(error) => Err(error.to_string()),
        None if status.is_success() => Err("missing token".into()),
        None => Err(format!("HTTP {status}")),
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("vergnify-agent/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

pub fn machine_hostname() -> String {
    let output = std::process::Command::new(hostname_program()).output();
    let Ok(output) = output else {
        return "unknown".into();
    };
    if !output.status.success() {
        return "unknown".into();
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        "unknown".into()
    } else {
        name
    }
}

fn hostname_program() -> &'static str {
    if cfg!(windows) {
        "hostname.exe"
    } else {
        "hostname"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_json_keeps_the_host_and_tools() {
        let inventory = Inventory {
            hostname: "mac".into(),
            os: "macos aarch64".into(),
            tools: Vec::new(),
        };
        let json = serde_json::to_value(&inventory).unwrap();
        assert_eq!(json["hostname"], "mac");
        assert_eq!(json["os"], "macos aarch64");
        assert!(json["tools"].as_array().unwrap().is_empty());
    }
}
