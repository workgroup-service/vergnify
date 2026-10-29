// @author Cursor

mod theme;

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    actions, div, img, prelude::*, px, rgb, size, App, Application, AssetSource, Bounds,
    ClipboardItem, Context, Div, FontWeight, Global, KeyBinding, Menu, MenuItem, SharedString,
    Subscription, TitlebarOptions, Window, WindowAppearance, WindowBounds, WindowHandle,
    WindowOptions,
};

actions!(vergnify, [OpenPreferences, Enroll, Reload, Quit]);
use theme::{Palette, ThemeChoice};
use vergnify_core::{ScanReport, Severity, ToolItem, VersionStatus, Vulnerability};

struct AppearancePreference(ThemeChoice);

impl Global for AppearancePreference {}

struct PreferencesWindow(Option<WindowHandle<Preferences>>);

impl Global for PreferencesWindow {}

struct EnrollWindow(Option<WindowHandle<EnrollView>>);

impl Global for EnrollWindow {}

struct Toolbox {
    loading: bool,
    report: Option<ScanReport>,
    _subscriptions: (Subscription, Subscription),
}

impl Toolbox {
    fn new(subscriptions: (Subscription, Subscription), cx: &mut Context<Self>) -> Self {
        Self::load(cx, false);
        Self {
            loading: true,
            report: None,
            _subscriptions: subscriptions,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        Self::load(cx, true);
    }

    fn load(cx: &mut Context<Self>, send_after: bool) {
        cx.spawn(async move |this, cx| {
            let report = cx
                .background_spawn(async move {
                    let report = scan();
                    if send_after && agent_auth().is_ok() {
                        if let Err(error) = send_scanned(&report) {
                            eprintln!("{}", vergnify_core::i18n::agent_send_failed(&error));
                        }
                    }
                    report
                })
                .await;
            this.update(cx, |view, cx| {
                view.report = Some(report);
                view.loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_vuln_window(
        &mut self,
        tool_name: String,
        vulns: Vec<Vulnerability>,
        cx: &mut Context<Self>,
    ) {
        let bounds = Bounds::centered(None, size(px(480.), px(520.)), cx);
        let title = vergnify_core::i18n::vuln_window_title(&tool_name);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(title)),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(360.), px(280.))),
                ..Default::default()
            },
            move |window, cx| {
                let subscriptions = watch_appearance(window, cx);
                cx.new(|_| VulnList {
                    tool_name,
                    vulns,
                    _subscriptions: subscriptions,
                })
            },
        )
        .ok();
        cx.activate(true);
    }
}

fn scan() -> ScanReport {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(vergnify_core::check(false))
}

fn watch_appearance(window: &mut Window, cx: &mut App) -> (Subscription, Subscription) {
    let appearance = window.observe_window_appearance(|window, _cx| {
        window.refresh();
    });
    let preference = window.observe_global::<AppearancePreference>(cx, |window, _cx| {
        window.refresh();
    });
    (appearance, preference)
}

fn colors(window: &Window, cx: &App) -> (ThemeChoice, Palette) {
    let choice = cx.global::<AppearancePreference>().0;
    let dark = match choice {
        ThemeChoice::Light => false,
        ThemeChoice::Dark => true,
        ThemeChoice::System => matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    };
    (choice, theme::palette(dark))
}

fn set_theme(choice: ThemeChoice, cx: &mut App) {
    theme::save(choice);
    cx.set_global(AppearancePreference(choice));
}

impl Render for Toolbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = match (&self.report, self.loading) {
            (_, true) => vergnify_core::i18n::loading().to_string(),
            (Some(report), false) => vergnify_core::i18n::tool_count(report.tools.len()),
            (None, false) => vergnify_core::i18n::not_loaded().to_string(),
        };
        let (_, palette) = colors(window, cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.text))
            .child(header(&summary, palette, cx))
            .child(body(self, palette, cx))
    }
}

fn header(summary: &str, palette: Palette, cx: &mut Context<Toolbox>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_4()
        .py_3()
        .bg(rgb(palette.card))
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child("Vergnify"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(rgb(palette.muted))
                .child(vergnify_core::i18n::compare_summary(summary))
                .child(reload_button(cx)),
        )
}

fn reload_button(cx: &mut Context<Toolbox>) -> impl IntoElement {
    div()
        .id("reload")
        .cursor_pointer()
        .child("🔁")
        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))
}

fn body(view: &Toolbox, palette: Palette, cx: &mut Context<Toolbox>) -> gpui::Stateful<Div> {
    let report = view.report.clone();
    let loading = view.loading;

    let mut list = div()
        .id("tools")
        .flex_1()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .overflow_scroll();

    if loading && report.is_none() {
        list = list.child(
            div()
                .text_color(rgb(palette.muted))
                .child(vergnify_core::i18n::checking_tools()),
        );
    } else if let Some(report) = report {
        if report.tools.is_empty() {
            list = list.child(
                div()
                    .text_color(rgb(palette.muted))
                    .child(vergnify_core::i18n::no_tools()),
            );
        } else {
            for tool in &report.tools {
                list = list.child(tool_row(tool, palette, cx));
            }
        }
        if !report.warnings.is_empty() {
            list = list.child(warnings(&report.warnings, palette));
        }
    }

    list
}

fn tool_row(tool: &ToolItem, palette: Palette, cx: &mut Context<Toolbox>) -> Div {
    let (label, foreground, background) = badge_colors(tool.status, palette);
    let newer = format_newer(tool);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(rgb(palette.card))
        .rounded_md()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(tool_icon(&tool.id, palette))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(tool.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(palette.muted))
                                        .child(shown(&tool.line)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(version_block(
                            vergnify_core::i18n::current_version(),
                            &tool.current_version,
                            palette,
                        ))
                        .child(div().text_color(rgb(palette.muted)).child("→"))
                        .child(version_block(
                            vergnify_core::i18n::latest_in_line(),
                            &tool.latest_version,
                            palette,
                        ))
                        .child(badge(label, foreground, background)),
                ),
        )
        .when(!tool.vulnerabilities.is_empty(), |row| {
            row.child(vuln_line(tool, palette, cx))
        })
        .when(!newer.is_empty(), |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child(vergnify_core::i18n::newer_lines(&newer)),
            )
        })
}

fn vuln_line(tool: &ToolItem, palette: Palette, cx: &mut Context<Toolbox>) -> Div {
    let mut line = div().flex().flex_wrap().gap_2().items_center();
    for (index, vuln) in tool.vulnerabilities.iter().take(4).enumerate() {
        line = line.child(cve_link(
            format!("cve-{}-{index}", tool.id),
            vuln,
            palette,
            cx,
        ));
    }
    let rest = tool.vulnerabilities.len().saturating_sub(4);
    if rest > 0 {
        let tool_name = tool.name.clone();
        let vulns = tool.vulnerabilities.clone();
        line = line.child(
            div()
                .id(SharedString::from(format!("cve-more-{}", tool.id)))
                .text_xs()
                .text_color(rgb(palette.red))
                .underline()
                .cursor_pointer()
                .child(format!("+{rest}"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_vuln_window(tool_name.clone(), vulns.clone(), cx);
                })),
        );
    }
    line
}

fn cve_link(
    element_id: String,
    vuln: &Vulnerability,
    palette: Palette,
    cx: &mut Context<Toolbox>,
) -> gpui::AnyElement {
    let cve_id = vuln.cve_id.clone();
    let label = format!("{} {}", vuln.cve_id, severity_ja(vuln.severity));
    div()
        .id(SharedString::from(element_id))
        .text_xs()
        .text_color(rgb(palette.red))
        .underline()
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |_, _, _, cx| open_vuln(&cve_id, cx)))
        .into_any_element()
}

fn open_vuln(id: &str, cx: &mut App) {
    if let Some(url) = osv_url(id) {
        cx.open_url(&url);
    }
}

fn osv_url(id: &str) -> Option<String> {
    let safe = !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    safe.then(|| format!("https://osv.dev/vulnerability/{id}"))
}

struct VulnList {
    tool_name: String,
    vulns: Vec<Vulnerability>,
    _subscriptions: (Subscription, Subscription),
}

impl Render for VulnList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (_, palette) = colors(window, cx);
        let vulns = self.vulns.clone();
        let mut list = div()
            .id("vuln-list")
            .flex_1()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .overflow_scroll();
        for (index, vuln) in vulns.iter().enumerate() {
            let cve_id = vuln.cve_id.clone();
            let title = if vuln.title == vuln.cve_id {
                String::new()
            } else {
                vuln.title.clone()
            };
            list = list.child(
                div()
                    .id(SharedString::from(format!("vuln-{index}")))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .bg(rgb(palette.card))
                    .rounded_md()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| open_vuln(&cve_id, cx)))
                    .child(
                        div()
                            .text_color(rgb(palette.red))
                            .underline()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{}  {}", vuln.cve_id, severity_ja(vuln.severity))),
                    )
                    .when(!title.is_empty(), |row| {
                        row.child(
                            div()
                                .text_sm()
                                .text_color(rgb(palette.muted))
                                .whitespace_normal()
                                .child(title),
                        )
                    }),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .px_4()
                    .py_3()
                    .bg(rgb(palette.card))
                    .text_sm()
                    .text_color(rgb(palette.muted))
                    .child(vergnify_core::i18n::vuln_window_hint(
                        &self.tool_name,
                        self.vulns.len(),
                    )),
            )
            .child(list)
    }
}

fn severity_ja(severity: Severity) -> &'static str {
    vergnify_core::i18n::severity_word(severity)
}

/// Logos sit on a light tile so dark marks stay readable.
/// Python is the official two-snake mark. The other marks are Devicon (MIT).
fn tool_icon(id: &str, palette: Palette) -> Div {
    let tile = div()
        .flex_shrink_0()
        .size(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(rgb(palette.icon_bg));
    if icon_bytes(id).is_some() {
        tile.child(img(format!("icons/{id}.svg")).size(px(28.)))
    } else {
        let letter = id.chars().next().unwrap_or('?').to_ascii_uppercase();
        tile.child(
            div()
                .text_color(rgb(palette.icon_text))
                .font_weight(FontWeight::BOLD)
                .child(letter.to_string()),
        )
    }
}

fn version_block(caption: &str, version: &str, palette: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .items_end()
        .child(
            div()
                .text_xs()
                .text_color(rgb(palette.muted))
                .child(caption.to_string()),
        )
        .child(div().child(shown(version)))
}

fn badge(label: &'static str, foreground: u32, background: u32) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded_md()
        .text_xs()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .child(label)
}

fn warnings(messages: &[String], palette: Palette) -> Div {
    let mut block = div().flex().flex_col().gap_1().pt_2();
    for message in messages {
        block = block.child(
            div()
                .text_xs()
                .text_color(rgb(palette.orange))
                .child(format!("warning: {message}")),
        );
    }
    block
}

fn badge_colors(status: VersionStatus, palette: Palette) -> (&'static str, u32, u32) {
    let label = vergnify_core::i18n::status_badge(status);
    match status {
        VersionStatus::UpToDate => (label, palette.green, palette.green_bg),
        VersionStatus::Outdated => (label, palette.orange, palette.orange_bg),
        VersionStatus::Unsupported => (label, palette.red, palette.red_bg),
        VersionStatus::Unknown => (label, palette.gray, palette.gray_bg),
    }
}

fn format_newer(tool: &ToolItem) -> String {
    tool.newer
        .iter()
        .map(|line| format!("{} ({})", line.label, line.latest))
        .collect::<Vec<_>>()
        .join(", ")
}

fn shown(value: &str) -> String {
    if value.is_empty() {
        "—".to_string()
    } else {
        value.to_string()
    }
}

struct Preferences {
    _subscriptions: (Subscription, Subscription),
}

impl Render for Preferences {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (choice, palette) = colors(window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.text))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(palette.muted))
                    .child(vergnify_core::i18n::view_settings()),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(vergnify_core::i18n::appearance()),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(appearance_choice(
                        "appearance-system",
                        ThemeChoice::System,
                        choice,
                        palette,
                        cx,
                    ))
                    .child(appearance_choice(
                        "appearance-light",
                        ThemeChoice::Light,
                        choice,
                        palette,
                        cx,
                    ))
                    .child(appearance_choice(
                        "appearance-dark",
                        ThemeChoice::Dark,
                        choice,
                        palette,
                        cx,
                    )),
            )
    }
}

fn appearance_choice(
    id: &'static str,
    choice: ThemeChoice,
    selected: ThemeChoice,
    palette: Palette,
    cx: &mut Context<Preferences>,
) -> Div {
    let label = match choice {
        ThemeChoice::System => vergnify_core::i18n::theme_system(),
        ThemeChoice::Light => vergnify_core::i18n::theme_light(),
        ThemeChoice::Dark => vergnify_core::i18n::theme_dark(),
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .child(
            div()
                .id(id)
                .w(px(88.))
                .h(px(64.))
                .rounded_md()
                .overflow_hidden()
                .border_1()
                .border_color(rgb(if choice == selected {
                    palette.text
                } else {
                    palette.card
                }))
                .cursor_pointer()
                .child(appearance_preview(choice))
                .on_click(cx.listener(move |_, _, _, cx| set_theme(choice, cx))),
        )
        .child(div().text_xs().text_color(rgb(palette.muted)).child(label))
}

fn appearance_preview(choice: ThemeChoice) -> Div {
    let light = theme::palette(false);
    let dark = theme::palette(true);
    match choice {
        ThemeChoice::System => div()
            .flex()
            .size_full()
            .child(div().flex_1().h_full().bg(rgb(light.bg)))
            .child(div().flex_1().h_full().bg(rgb(dark.bg))),
        ThemeChoice::Light => div().size_full().bg(rgb(light.bg)),
        ThemeChoice::Dark => div().size_full().bg(rgb(dark.bg)),
    }
}

fn open_preferences(cx: &mut App) {
    if let Some(existing) = cx.try_global::<PreferencesWindow>().and_then(|slot| slot.0) {
        if existing
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(420.), px(240.)), cx);
    let title = vergnify_core::i18n::preferences()
        .trim_end_matches('…')
        .to_string();
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(title)),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(360.), px(220.))),
                ..Default::default()
            },
            |window, cx| {
                let subscriptions = watch_appearance(window, cx);
                cx.new(|_| Preferences {
                    _subscriptions: subscriptions,
                })
            },
        )
        .unwrap();
    cx.set_global(PreferencesWindow(Some(window)));
    cx.activate(true);
}

enum EnrollState {
    Working,
    Waiting { code: String, url: String },
    Done { enrolled: String, sent: String },
    Failed(String),
}

struct EnrollView {
    state: EnrollState,
    copied: bool,
    _subscriptions: (Subscription, Subscription),
}

impl EnrollView {
    fn begin(&mut self, cx: &mut Context<Self>) {
        self.state = EnrollState::Working;
        self.copied = false;
        cx.notify();
        let Some(url) = admin_url() else {
            self.state = EnrollState::Failed(vergnify_core::i18n::agent_missing_url().to_string());
            return;
        };
        cx.spawn(async move |this, cx| {
            let started = cx
                .background_spawn({
                    let url = url.clone();
                    async move { block_on(vergnify_agent::begin_enrollment(&url)).flatten() }
                })
                .await;
            let enrollment = match started {
                Ok(enrollment) => enrollment,
                Err(error) => {
                    this.update(cx, |view, cx| {
                        view.state =
                            EnrollState::Failed(vergnify_core::i18n::agent_send_failed(&error));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let code = enrollment.user_code.clone();
            let verification = enrollment.verification_uri.clone();
            this.update(cx, |view, cx| {
                view.state = EnrollState::Waiting {
                    code: code.clone(),
                    url: verification.clone(),
                };
                cx.notify();
            })
            .ok();
            let deadline = Instant::now() + Duration::from_secs(15 * 60);
            loop {
                if Instant::now() > deadline {
                    this.update(cx, |view, cx| {
                        view.state =
                            EnrollState::Failed(vergnify_core::i18n::agent_send_failed(&"expired"));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                let polled = cx
                    .background_spawn({
                        let url = url.clone();
                        let device_code = enrollment.device_code.clone();
                        async move {
                            block_on(vergnify_agent::poll_enrollment(&url, &device_code)).flatten()
                        }
                    })
                    .await;
                match polled {
                    Ok(Some(token)) => {
                        if let Err(error) =
                            vergnify_agent::save_credential(&vergnify_agent::Credential {
                                admin_url: url.clone(),
                                token,
                            })
                        {
                            this.update(cx, |view, cx| {
                                view.state = EnrollState::Failed(
                                    vergnify_core::i18n::agent_send_failed(&error),
                                );
                                cx.notify();
                            })
                            .ok();
                            return;
                        }
                        let sent = cx.background_spawn(async move { send_inventory() }).await;
                        let sent_ok = sent.is_ok();
                        this.update(cx, |view, cx| {
                            view.state = match sent {
                                Ok(hostname) => EnrollState::Done {
                                    enrolled: vergnify_core::i18n::agent_enrolled().to_string(),
                                    sent: vergnify_core::i18n::agent_sent(&hostname),
                                },
                                Err(error) => EnrollState::Failed(
                                    vergnify_core::i18n::agent_send_failed(&error),
                                ),
                            };
                            if sent_ok {
                                start_inventory_loop(cx, true);
                            }
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    Ok(None) => {
                        cx.background_executor().timer(enrollment.interval).await;
                    }
                    Err(error) => {
                        this.update(cx, |view, cx| {
                            view.state =
                                EnrollState::Failed(vergnify_core::i18n::agent_send_failed(&error));
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                }
            }
        })
        .detach();
    }
}

impl Render for EnrollView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (_, palette) = colors(window, cx);
        let body = match &self.state {
            EnrollState::Working => div()
                .text_color(rgb(palette.muted))
                .child(vergnify_core::i18n::agent_enroll_help()),
            EnrollState::Waiting { code, url } => {
                let code = code.clone();
                let copied = self.copied;
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(code.clone()),
                            )
                            .child(copy_code_button(code.clone(), copied, palette, cx)),
                    )
                    .child(
                        div()
                            .text_color(rgb(palette.muted))
                            .child(vergnify_core::i18n::agent_enroll_code(&code, url)),
                    )
            }
            EnrollState::Done { enrolled, sent } => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(enrolled.clone())
                .child(sent.clone()),
            EnrollState::Failed(message) => {
                div().text_color(rgb(palette.red)).child(message.clone())
            }
        };
        div()
            .size_full()
            .p_5()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.text))
            .child(body)
    }
}

fn open_enroll(cx: &mut App) {
    if let Some(existing) = cx.try_global::<EnrollWindow>().and_then(|slot| slot.0) {
        let restarted = existing
            .update(cx, |view, window, cx| {
                window.activate_window();
                let restart = matches!(
                    view.state,
                    EnrollState::Done { .. } | EnrollState::Failed(_)
                );
                if restart {
                    view.begin(cx);
                }
            })
            .is_ok();
        if restarted {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(480.), px(220.)), cx);
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from(vergnify_core::i18n::enroll())),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(360.), px(180.))),
                ..Default::default()
            },
            |window, cx| {
                let subscriptions = watch_appearance(window, cx);
                cx.new(|cx| {
                    let mut view = EnrollView {
                        state: EnrollState::Working,
                        copied: false,
                        _subscriptions: subscriptions,
                    };
                    view.begin(cx);
                    view
                })
            },
        )
        .unwrap();
    cx.set_global(EnrollWindow(Some(window)));
    cx.activate(true);
}

fn admin_url() -> Option<String> {
    std::env::var("VERGNIFY_ADMIN_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| vergnify_agent::load_credential().map(|credential| credential.admin_url))
}

fn agent_auth() -> Result<(String, String), String> {
    let stored = vergnify_agent::load_credential();
    let url = admin_url().ok_or_else(|| vergnify_core::i18n::agent_missing_url().to_string())?;
    let token = std::env::var("VERGNIFY_AGENT_TOKEN")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| stored.map(|credential| credential.token))
        .ok_or_else(|| vergnify_core::i18n::agent_missing_token().to_string())?;
    Ok((url, token))
}

fn send_scanned(report: &vergnify_core::ScanReport) -> Result<(), String> {
    let (url, token) = agent_auth()?;
    let inventory = vergnify_agent::Inventory {
        hostname: vergnify_agent::machine_hostname(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        tools: report.tools.clone(),
    };
    block_on(vergnify_agent::send(&url, &token, &inventory))?
}

fn send_inventory() -> Result<String, String> {
    let (url, token) = agent_auth()?;
    block_on(async {
        let inventory = vergnify_agent::collect().await;
        vergnify_agent::send(&url, &token, &inventory).await?;
        Ok(inventory.hostname)
    })?
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    Ok(runtime.block_on(future))
}

fn sync_interval() -> Duration {
    let seconds = std::env::var("VERGNIFY_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds >= 60)
        .unwrap_or(15 * 60);
    Duration::from_secs(seconds)
}

fn start_inventory_loop(cx: &App, delay_first: bool) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    cx.background_executor()
        .spawn(async move {
            if delay_first {
                std::thread::sleep(sync_interval());
            }
            loop {
                if let Err(error) = send_inventory() {
                    eprintln!("{}", vergnify_core::i18n::agent_send_failed(&error));
                }
                std::thread::sleep(sync_interval());
            }
        })
        .detach();
}

fn copy_code_button(
    code: String,
    copied: bool,
    palette: Palette,
    cx: &mut Context<EnrollView>,
) -> impl IntoElement {
    let color = if copied { palette.green } else { palette.muted };
    div()
        .id("copy-enroll-code")
        .w(px(28.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(copy_icon(color, palette.bg))
        .on_click(cx.listener(move |view, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
            view.copied = true;
            cx.notify();
        }))
}

fn copy_icon(color: u32, fill: u32) -> Div {
    div()
        .relative()
        .w(px(16.))
        .h(px(16.))
        .child(
            div()
                .absolute()
                .top(px(0.))
                .right(px(0.))
                .w(px(11.))
                .h(px(11.))
                .rounded_sm()
                .border_1()
                .border_color(rgb(color)),
        )
        .child(
            div()
                .absolute()
                .bottom(px(0.))
                .left(px(0.))
                .w(px(11.))
                .h(px(11.))
                .rounded_sm()
                .border_1()
                .border_color(rgb(color))
                .bg(rgb(fill)),
        )
}

#[cfg(target_os = "macos")]
fn use_menu_bar_name() {
    if std::env::current_exe().ok().is_some_and(|path| {
        path.components()
            .any(|component| component.as_os_str() == "Vergnify.app")
    }) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let contents = dir.join("Vergnify.app/Contents");
    let macos = contents.join("MacOS");
    use std::os::unix::process::CommandExt;
    if std::fs::create_dir_all(&macos).is_err() {
        return;
    }
    let _ = std::fs::write(contents.join("Info.plist"), VERGNIFY_INFO_PLIST);
    let _ = std::fs::write(contents.join("PkgInfo"), "APPL????");
    let bundled = macos.join("Vergnify");
    if !same_inode(&exe, &bundled) {
        let _ = std::fs::remove_file(&bundled);
        if std::fs::hard_link(&exe, &bundled).is_err() && std::fs::copy(&exe, &bundled).is_err() {
            return;
        }
    }
    let _ = std::process::Command::new(bundled)
        .args(std::env::args().skip(1))
        .exec();
}

#[cfg(target_os = "macos")]
const VERGNIFY_INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Vergnify</string>
  <key>CFBundleDisplayName</key>
  <string>Vergnify</string>
  <key>CFBundleExecutable</key>
  <string>Vergnify</string>
  <key>CFBundleIdentifier</key>
  <string>com.vergnify.gui</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#;

#[cfg(target_os = "macos")]
fn same_inode(left: &std::path::Path, right: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(left) = std::fs::metadata(left) else {
        return false;
    };
    let Ok(right) = std::fs::metadata(right) else {
        return false;
    };
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(target_os = "macos"))]
fn use_menu_bar_name() {}

fn install_menus(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-,", OpenPreferences, None),
        KeyBinding::new("cmd-r", Reload, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    cx.set_menus(vec![Menu {
        name: "Vergnify".into(),
        items: vec![
            MenuItem::action(vergnify_core::i18n::preferences(), OpenPreferences),
            MenuItem::action(vergnify_core::i18n::enroll(), Enroll),
            MenuItem::separator(),
            MenuItem::action(vergnify_core::i18n::quit_app(), Quit),
        ],
    }]);
}

fn main() {
    use_menu_bar_name();
    Application::new().with_assets(Icons).run(|cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        cx.set_global(AppearancePreference(theme::load()));
        install_menus(cx);
        cx.on_action(|_: &OpenPreferences, cx| open_preferences(cx));
        cx.on_action(|_: &Enroll, cx| open_enroll(cx));
        cx.on_action(|_: &Quit, cx| cx.quit());
        if agent_auth().is_ok() {
            start_inventory_loop(cx, false);
        }

        let bounds = Bounds::centered(None, size(px(920.), px(680.)), cx);
        let main = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Vergnify")),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(640.), px(420.))),
                    ..Default::default()
                },
                |window, cx| {
                    let subscriptions = watch_appearance(window, cx);
                    cx.new(|cx| Toolbox::new(subscriptions, cx))
                },
            )
            .unwrap();
        cx.on_action(move |_: &Reload, cx| {
            main.update(cx, |this, _, cx| this.refresh(cx)).ok();
        });
        cx.activate(true);
    });
}

fn icon_bytes(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "python" => include_bytes!("../assets/icons/python.svg"),
        "node" => include_bytes!("../assets/icons/node.svg"),
        "deno" => include_bytes!("../assets/icons/deno.svg"),
        "bun" => include_bytes!("../assets/icons/bun.svg"),
        "php" => include_bytes!("../assets/icons/php.svg"),
        "go" => include_bytes!("../assets/icons/go.svg"),
        "rust" => include_bytes!("../assets/icons/rust.svg"),
        "ruby" => include_bytes!("../assets/icons/ruby.svg"),
        "elixir" => include_bytes!("../assets/icons/elixir.svg"),
        "java" => include_bytes!("../assets/icons/java.svg"),
        "dotnet" => include_bytes!("../assets/icons/dotnet.svg"),
        "swift" => include_bytes!("../assets/icons/swift.svg"),
        "flutter" => include_bytes!("../assets/icons/flutter.svg"),
        "docker" => include_bytes!("../assets/icons/docker.svg"),
        "podman" => include_bytes!("../assets/icons/podman.svg"),
        "git" => include_bytes!("../assets/icons/git.svg"),
        "gh" => include_bytes!("../assets/icons/gh.svg"),
        "terraform" => include_bytes!("../assets/icons/terraform.svg"),
        "kubectl" => include_bytes!("../assets/icons/kubectl.svg"),
        "helm" => include_bytes!("../assets/icons/helm.svg"),
        "aws" => include_bytes!("../assets/icons/aws.svg"),
        "az" => include_bytes!("../assets/icons/az.svg"),
        "uv" => include_bytes!("../assets/icons/uv.svg"),
        _ => return None,
    })
}

struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let Some(id) = path
            .strip_prefix("icons/")
            .and_then(|name| name.strip_suffix(".svg"))
        else {
            return Ok(None);
        };
        Ok(icon_bytes(id).map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
