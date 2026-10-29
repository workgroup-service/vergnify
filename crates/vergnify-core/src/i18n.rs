// @author Cursor

use std::sync::OnceLock;

use crate::model::{Severity, VersionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

pub fn active() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(|| {
        sys_locale::get_locale()
            .as_deref()
            .map(lang_from_tag)
            .unwrap_or(Lang::En)
    })
}

pub fn lang_from_tag(tag: &str) -> Lang {
    let tag = tag.trim().to_ascii_lowercase().replace('_', "-");
    if tag == "ja" || tag.starts_with("ja-") {
        Lang::Ja
    } else {
        Lang::En
    }
}

pub fn loading() -> &'static str {
    match active() {
        Lang::Ja => "読み込み中",
        Lang::En => "Loading",
    }
}

pub fn not_loaded() -> &'static str {
    match active() {
        Lang::Ja => "未取得",
        Lang::En => "Not loaded",
    }
}

pub fn reload() -> &'static str {
    match active() {
        Lang::Ja => "再読み込み",
        Lang::En => "Reload",
    }
}

pub fn checking_tools() -> &'static str {
    match active() {
        Lang::Ja => "インストール済みのツールを確認しています。",
        Lang::En => "Checking installed tools.",
    }
}

pub fn no_tools() -> &'static str {
    match active() {
        Lang::Ja => "インストール済みの対象ツールは見つかりませんでした。",
        Lang::En => "No supported tools were found.",
    }
}

pub fn current_version() -> &'static str {
    match active() {
        Lang::Ja => "現在",
        Lang::En => "Current",
    }
}

pub fn latest_in_line() -> &'static str {
    match active() {
        Lang::Ja => "系列の最新",
        Lang::En => "Latest in line",
    }
}

pub fn preferences() -> &'static str {
    match active() {
        Lang::Ja => "設定…",
        Lang::En => "Preferences…",
    }
}

pub fn enroll() -> &'static str {
    match active() {
        Lang::Ja => "登録",
        Lang::En => "Enroll",
    }
}

pub fn quit_app() -> &'static str {
    match active() {
        Lang::Ja => "Vergnify を終了",
        Lang::En => "Quit Vergnify",
    }
}

pub fn view_settings() -> &'static str {
    match active() {
        Lang::Ja => "表示",
        Lang::En => "View",
    }
}

pub fn appearance() -> &'static str {
    match active() {
        Lang::Ja => "外観",
        Lang::En => "Appearance",
    }
}

pub fn theme_system() -> &'static str {
    match active() {
        Lang::Ja => "システム",
        Lang::En => "System",
    }
}

pub fn theme_light() -> &'static str {
    match active() {
        Lang::Ja => "ライト",
        Lang::En => "Light",
    }
}

pub fn theme_dark() -> &'static str {
    match active() {
        Lang::Ja => "ダーク",
        Lang::En => "Dark",
    }
}

pub fn update_button() -> &'static str {
    match active() {
        Lang::Ja => "更新",
        Lang::En => "Update",
    }
}

pub fn status_badge(status: VersionStatus) -> &'static str {
    match (active(), status) {
        (Lang::Ja, VersionStatus::UpToDate) => "最新",
        (Lang::Ja, VersionStatus::Outdated) => "更新あり",
        (Lang::Ja, VersionStatus::Unsupported) => "サポート終了",
        (Lang::Ja, VersionStatus::Unknown) => "不明",
        (Lang::En, VersionStatus::UpToDate) => "Up to date",
        (Lang::En, VersionStatus::Outdated) => "Update available",
        (Lang::En, VersionStatus::Unsupported) => "Unsupported",
        (Lang::En, VersionStatus::Unknown) => "Unknown",
    }
}

pub fn severity_word(severity: Severity) -> &'static str {
    match (active(), severity) {
        (Lang::Ja, Severity::Critical) => "重大",
        (Lang::Ja, Severity::High) => "高",
        (Lang::Ja, Severity::Medium) => "中",
        (Lang::Ja, Severity::Low) => "低",
        (Lang::En, Severity::Critical) => "Critical",
        (Lang::En, Severity::High) => "High",
        (Lang::En, Severity::Medium) => "Medium",
        (Lang::En, Severity::Low) => "Low",
    }
}

pub fn tool_count(count: usize) -> String {
    tool_count_for(active(), count)
}

fn tool_count_for(lang: Lang, count: usize) -> String {
    match lang {
        Lang::Ja => format!("{count} 件"),
        Lang::En if count == 1 => "1 tool".into(),
        Lang::En => format!("{count} tools"),
    }
}

pub fn compare_summary(summary: &str) -> String {
    match active() {
        Lang::Ja => format!("同じ系列の最新パッチと比較  ·  {summary}"),
        Lang::En => format!("Compared with the latest patch in the same line  ·  {summary}"),
    }
}

pub fn newer_lines(newer: &str) -> String {
    match active() {
        Lang::Ja => format!("新しい系列: {newer}"),
        Lang::En => format!("Newer lines: {newer}"),
    }
}

pub fn updating(name: &str, manager: &str) -> String {
    match active() {
        Lang::Ja => format!("{name} を {manager} で更新しています"),
        Lang::En => format!("Updating {name} with {manager}"),
    }
}

pub fn vuln_window_title(name: &str) -> String {
    match active() {
        Lang::Ja => format!("{name} の脆弱性"),
        Lang::En => format!("{name} vulnerabilities"),
    }
}

pub fn vuln_window_hint(name: &str, count: usize) -> String {
    vuln_window_hint_for(active(), name, count)
}

fn vuln_window_hint_for(lang: Lang, name: &str, count: usize) -> String {
    match lang {
        Lang::Ja => format!("{name} の {count} 件。クリックするとブラウザで開きます。"),
        Lang::En if count == 1 => {
            format!("{name}: 1 vulnerability. Click to open in the browser.")
        }
        Lang::En => format!("{name}: {count} vulnerabilities. Click to open in the browser."),
    }
}

pub fn cli_about() -> &'static str {
    match active() {
        Lang::Ja => "インストール済みランタイムと CLI のバージョンと脆弱性を確認する",
        Lang::En => "Show versions and known vulnerabilities for installed runtimes and CLIs",
    }
}

pub fn cli_after_help() -> &'static str {
    match active() {
        Lang::Ja => {
            "例:\n  vergnify list\n  vergnify check\n  vergnify check --offline\n  vergnify update node\n  vergnify update --dry-run"
        }
        Lang::En => {
            "Examples:\n  vergnify list\n  vergnify check\n  vergnify check --offline\n  vergnify update node\n  vergnify update --dry-run"
        }
    }
}

pub fn help_json() -> &'static str {
    match active() {
        Lang::Ja => "人間向けの表ではなく JSON を出力する",
        Lang::En => "Print JSON instead of a human-readable table",
    }
}

pub fn help_list() -> &'static str {
    match active() {
        Lang::Ja => "インストール済みツールの現在バージョンを表示する",
        Lang::En => "Show the current version of installed tools",
    }
}

pub fn help_check() -> &'static str {
    match active() {
        Lang::Ja => "入っている系列の最新パッチと比べ、より新しい系列を表示する",
        Lang::En => "Compare each installed line with its latest patch and list newer lines",
    }
}

pub fn help_offline() -> &'static str {
    match active() {
        Lang::Ja => "ネットワークに接続せず、24時間キャッシュだけを使う",
        Lang::En => "Use the 24-hour cache and do not use the network",
    }
}

pub fn help_update() -> &'static str {
    match active() {
        Lang::Ja => "homebrew / winget / scoop / apt で更新する",
        Lang::En => "Update through Homebrew, winget, scoop, or apt",
    }
}

pub fn help_tool() -> &'static str {
    match active() {
        Lang::Ja => "更新するツールの id または名前。省略すると古いものだけを更新する",
        Lang::En => "Tool id or name to update. Omit to update outdated and unsupported tools",
    }
}

pub fn help_dry_run() -> &'static str {
    match active() {
        Lang::Ja => "コマンドを表示するだけで実行しない",
        Lang::En => "Print the command without running it",
    }
}

pub fn json_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("JSON の出力に失敗しました ({error})"),
        Lang::En => format!("Failed to write JSON ({error})"),
    }
}

pub fn nothing_to_update() -> &'static str {
    match active() {
        Lang::Ja => "更新するツールはありません。",
        Lang::En => "Nothing to update.",
    }
}

pub fn offline_latest_unknown() -> &'static str {
    match active() {
        Lang::Ja => "オフラインのため、キャッシュにないツールの最新バージョンは不明です。",
        Lang::En => "Offline mode: latest versions missing from the cache are unknown.",
    }
}

pub fn http_client_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("HTTP クライアントを初期化できませんでした ({error})"),
        Lang::En => format!("Could not create the HTTP client ({error})"),
    }
}

pub fn latest_task_failed() -> &'static str {
    match active() {
        Lang::Ja => "最新バージョンの取得タスクが失敗しました。",
        Lang::En => "A task that fetches latest versions failed.",
    }
}

pub fn latest_used_cache(id: &str, error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("{id}: 最新の取得に失敗したためキャッシュを使いました ({error})"),
        Lang::En => {
            format!("{id}: used the cache because fetching the latest version failed ({error})")
        }
    }
}

pub fn latest_fetch_failed(id: &str, error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("{id}: 最新バージョンを取得できませんでした ({error})"),
        Lang::En => format!("{id}: could not fetch the latest version ({error})"),
    }
}

pub fn latest_cache_save_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("最新バージョンのキャッシュを保存できませんでした ({error})"),
        Lang::En => format!("Could not save the latest-version cache ({error})"),
    }
}

pub fn offline_vulns_cache_only() -> &'static str {
    match active() {
        Lang::Ja => "オフラインのため、脆弱性はキャッシュにあるものだけを表示します。",
        Lang::En => "Offline mode: vulnerabilities are shown only when they are cached.",
    }
}

pub fn osv_client_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("OSV.dev の HTTP クライアントを初期化できませんでした ({error})"),
        Lang::En => format!("Could not create the OSV.dev HTTP client ({error})"),
    }
}

pub fn vuln_task_failed() -> &'static str {
    match active() {
        Lang::Ja => "脆弱性の取得タスクが失敗しました。",
        Lang::En => "A task that fetches vulnerabilities failed.",
    }
}

pub fn vuln_fetch_failed(name: &str, error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("{name} の脆弱性を OSV.dev から取得できませんでした ({error})"),
        Lang::En => format!("Could not fetch vulnerabilities for {name} from OSV.dev ({error})"),
    }
}

pub fn vuln_cache_save_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("脆弱性キャッシュを保存できませんでした ({error})"),
        Lang::En => format!("Could not save the vulnerability cache ({error})"),
    }
}

pub fn osv_package_missing() -> &'static str {
    match active() {
        Lang::Ja => "照合先がありません",
        Lang::En => "No vulnerability source is configured",
    }
}

pub fn notify_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("OS 通知を出せませんでした ({error})"),
        Lang::En => format!("Could not show an OS notification ({error})"),
    }
}

pub fn version_unreadable(name: &str, detail: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("{name}: バージョンを読み取れませんでした ({detail})"),
        Lang::En => format!("{name}: could not read a version ({detail})"),
    }
}

pub fn version_failed(name: &str, error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("{name}: バージョンを取得できませんでした ({error})"),
        Lang::En => format!("{name}: could not get the version ({error})"),
    }
}

pub fn dry_run_detail() -> &'static str {
    match active() {
        Lang::Ja => "実行せずコマンドだけを表示しました",
        Lang::En => "Printed the command without running it",
    }
}

pub fn brew_formula_missing(name: &str) -> String {
    match active() {
        Lang::Ja => {
            format!("{name} は Homebrew の配下にありますが、対応する formula が見つかりません")
        }
        Lang::En => format!("{name} is under Homebrew, but no matching formula was found"),
    }
}

pub fn unmanaged(name: &str) -> String {
    match active() {
        Lang::Ja => format!("{name} は homebrew / winget / scoop / apt の管理下にありません"),
        Lang::En => format!("{name} is not managed by Homebrew, winget, scoop, or apt"),
    }
}

pub fn target_missing(name: &str) -> String {
    match active() {
        Lang::Ja => format!("対象のツールが見つかりません ({name})"),
        Lang::En => format!("Tool not found ({name})"),
    }
}

pub fn updated() -> &'static str {
    match active() {
        Lang::Ja => "更新しました",
        Lang::En => "Updated",
    }
}

pub fn exit_code(status: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("終了コード {status}"),
        Lang::En => format!("Exit code {status}"),
    }
}

pub fn updated_with_detail(detail: &str) -> String {
    match active() {
        Lang::Ja => format!("更新しました ({detail})"),
        Lang::En => format!("Updated ({detail})"),
    }
}

pub fn agent_about() -> &'static str {
    match active() {
        Lang::Ja => "インストール済みツールの在庫を Admin へ送る",
        Lang::En => "Send the installed-tool inventory to the Admin server",
    }
}

pub fn agent_once_help() -> &'static str {
    match active() {
        Lang::Ja => "1回送って終了する",
        Lang::En => "Send once and exit",
    }
}

pub fn agent_missing_url() -> &'static str {
    match active() {
        Lang::Ja => "VERGNIFY_ADMIN_URL が必要です",
        Lang::En => "VERGNIFY_ADMIN_URL is required",
    }
}

pub fn agent_missing_token() -> &'static str {
    match active() {
        Lang::Ja => "先に --enroll するか、VERGNIFY_AGENT_TOKEN を設定してください",
        Lang::En => "Run with --enroll, or set VERGNIFY_AGENT_TOKEN",
    }
}

pub fn agent_enroll_help() -> &'static str {
    match active() {
        Lang::Ja => "この端末を Admin に登録する",
        Lang::En => "Register this computer with the Admin server",
    }
}

pub fn agent_enroll_code(code: &str, url: &str) -> String {
    match active() {
        Lang::Ja => format!("コード {code} を {url} で承認してください"),
        Lang::En => format!("Approve code {code} at {url}"),
    }
}

pub fn agent_enrolled() -> &'static str {
    match active() {
        Lang::Ja => "この端末を登録しました",
        Lang::En => "Registered this computer",
    }
}

pub fn agent_sent(hostname: &str) -> String {
    match active() {
        Lang::Ja => format!("{hostname} の在庫を送りました"),
        Lang::En => format!("Sent the inventory for {hostname}"),
    }
}

pub fn agent_send_failed(error: &impl std::fmt::Display) -> String {
    match active() {
        Lang::Ja => format!("在庫を送れませんでした ({error})"),
        Lang::En => format!("Could not send the inventory ({error})"),
    }
}

pub fn more_alerts(count: usize) -> String {
    more_alerts_for(active(), count)
}

fn more_alerts_for(lang: Lang, count: usize) -> String {
    match lang {
        Lang::Ja => format!("ほか {count} 件"),
        Lang::En => format!("and {count} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_tags_select_japanese() {
        assert_eq!(lang_from_tag("ja"), Lang::Ja);
        assert_eq!(lang_from_tag("ja-JP"), Lang::Ja);
        assert_eq!(lang_from_tag("ja_JP"), Lang::Ja);
        assert_eq!(lang_from_tag("en-US"), Lang::En);
        assert_eq!(lang_from_tag(""), Lang::En);
    }

    #[test]
    fn counts_use_english_plurals() {
        assert_eq!(tool_count_for(Lang::Ja, 1), "1 件");
        assert_eq!(tool_count_for(Lang::En, 1), "1 tool");
        assert_eq!(tool_count_for(Lang::En, 2), "2 tools");
        assert_eq!(
            vuln_window_hint_for(Lang::En, "Python", 1),
            "Python: 1 vulnerability. Click to open in the browser."
        );
        assert_eq!(
            vuln_window_hint_for(Lang::Ja, "Python", 2),
            "Python の 2 件。クリックするとブラウザで開きます。"
        );
        assert_eq!(more_alerts_for(Lang::Ja, 3), "ほか 3 件");
        assert_eq!(more_alerts_for(Lang::En, 3), "and 3 more");
    }
}
