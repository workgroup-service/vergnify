// @author Cursor

use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgAction, CommandFactory, FromArgMatches, Parser};
use vergnify_agent::Inventory;
use vergnify_core::i18n;

#[derive(Debug, Parser)]
#[command(name = "vergnify-agent")]
struct Cli {
    #[arg(long, action = ArgAction::SetTrue)]
    once: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    enroll: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let matches = Cli::command()
        .about(i18n::agent_about())
        .mut_arg("once", |arg| arg.help(i18n::agent_once_help()))
        .mut_arg("enroll", |arg| arg.help(i18n::agent_enroll_help()))
        .get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());

    let stored = vergnify_agent::load_credential();
    let Some(admin_url) = std::env::var("VERGNIFY_ADMIN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            stored
                .as_ref()
                .map(|credential| credential.admin_url.clone())
        })
    else {
        eprintln!("{}", i18n::agent_missing_url());
        return ExitCode::from(1);
    };
    if cli.enroll {
        return enroll(&admin_url).await;
    }
    let Some(token) = std::env::var("VERGNIFY_AGENT_TOKEN")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| stored.map(|credential| credential.token))
    else {
        eprintln!("{}", i18n::agent_missing_token());
        return ExitCode::from(1);
    };

    let interval = sync_interval();
    loop {
        match sync_once(&admin_url, &token).await {
            Ok(inventory) => println!("{}", i18n::agent_sent(&inventory.hostname)),
            Err(error) => {
                eprintln!("{}", i18n::agent_send_failed(&error));
                if cli.once {
                    return ExitCode::from(1);
                }
            }
        }
        if cli.once {
            return ExitCode::from(0);
        }
        tokio::time::sleep(interval).await;
    }
}

async fn enroll(admin_url: &str) -> ExitCode {
    let enrollment = match vergnify_agent::begin_enrollment(admin_url).await {
        Ok(enrollment) => enrollment,
        Err(error) => {
            eprintln!("{}", i18n::agent_send_failed(&error));
            return ExitCode::from(1);
        }
    };
    println!(
        "{}",
        i18n::agent_enroll_code(&enrollment.user_code, &enrollment.verification_uri)
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);
    loop {
        if std::time::Instant::now() > deadline {
            eprintln!("{}", i18n::agent_send_failed(&"expired"));
            return ExitCode::from(1);
        }
        match vergnify_agent::poll_enrollment(admin_url, &enrollment.device_code).await {
            Ok(Some(token)) => {
                if let Err(error) = vergnify_agent::save_credential(&vergnify_agent::Credential {
                    admin_url: admin_url.to_string(),
                    token,
                }) {
                    eprintln!("{}", i18n::agent_send_failed(&error));
                    return ExitCode::from(1);
                }
                println!("{}", i18n::agent_enrolled());
                return ExitCode::SUCCESS;
            }
            Ok(None) => tokio::time::sleep(enrollment.interval).await,
            Err(error) => {
                eprintln!("{}", i18n::agent_send_failed(&error));
                return ExitCode::from(1);
            }
        }
    }
}

async fn sync_once(admin_url: &str, token: &str) -> Result<Inventory, String> {
    let inventory = vergnify_agent::collect().await;
    vergnify_agent::send(admin_url, token, &inventory).await?;
    Ok(inventory)
}

fn sync_interval() -> Duration {
    let seconds = std::env::var("VERGNIFY_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds >= 60)
        .unwrap_or(15 * 60);
    Duration::from_secs(seconds)
}
