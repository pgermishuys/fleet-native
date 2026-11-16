//! The status bar's usage chip and its popover (component 9, piece J): `UsageLimitsPopover.test.ts` under its names (the
//! phone sheet's case is skipped: no phone layout), and `StatusBar.test.ts`'s "shows the usage chip unless the Settings
//! switch hides it". Fleet's tests mount the component; these draw the status bar and the popover it opens.
use crate::fleet::tests::usage_rules::{Rig, body, reads, usage_fleet};
use crate::fleet::usage::read_usage_limits;
use crate::mock::failure::NOW;
use crate::mock::usage::{at, provider, window};
use crate::tests::{FRAME, app, click};
use crate::ui::Open;
use rune::{ElementInfo, Runtime};
use serde_json::{Value, json};

fn info(rt: &Runtime<crate::State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    info(rt, key).unwrap_or_else(|| panic!("no element keyed {key:?}")).text.unwrap_or_default()
}

/// Everything a provider's part of the popover says, in reading order: `usage-…-<provider>-<machine>` keys.
fn said(rt: &Runtime<crate::State>, provider: &str, machine: &str) -> String {
    let id = format!("{provider}-{machine}");
    let mut found: Vec<ElementInfo> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting && e.text.is_some() && e.key.as_deref().is_some_and(|k| k.starts_with("usage-") && k.contains(&id)))
        .collect();
    found.sort_by(|a, b| (a.rect[1], a.rect[0]).partial_cmp(&(b.rect[1], b.rect[0])).unwrap());
    found.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join(" | ")
}

fn serve(providers: Vec<Value>) -> Runtime<crate::State> {
    let mut rt = app();
    set(&mut rt, providers);
    rt
}

fn set(rt: &mut Runtime<crate::State>, providers: Vec<Value>) {
    rt.update(move |s, _| {
        s.now_ms = Some(NOW);
        s.pickers.usage.limits = read_usage_limits(&json!({"enabled": true, "providers": providers})).expect("a body that reads");
    });
    rt.frame(FRAME);
}

fn open(providers: Vec<Value>) -> Runtime<crate::State> {
    let mut rt = serve(providers);
    click(&mut rt, "usage-limits-chip");
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt
}

fn claude(windows: Vec<Value>) -> Value {
    provider("claude", "Claude", json!({ "windows": windows }))
}

fn tone_of(rt: &Runtime<crate::State>) -> (rune::Color, Option<rune::Color>) {
    (info(rt, "usage-chip-name").unwrap().text_color.unwrap(), info(rt, "usage-limits-chip").map(|c| c.fill))
}

// UsageLimitsPopover.test.ts: "shows the tightest window on the chip: grey below 80%, amber above, red when used up"
#[test]
fn shows_the_tightest_window_on_the_chip_grey_below_80_amber_above_red_when_used_up() {
    let t = crate::theme::Theme::of(true);
    let rt = serve(vec![claude(vec![window("five_hour", 0.42, 2.0, None, "allowed")])]);
    assert_eq!(text_of(&rt, "usage-chip-name"), "Claude 5 h");
    assert_eq!(text_of(&rt, "usage-chip-percent"), "42%");
    assert_eq!(tone_of(&rt).0, t.muted, "quiet");
    assert_eq!(tone_of(&rt).1.unwrap().a, 0.0, "no fill");

    let rt = serve(vec![claude(vec![window("five_hour", 0.85, 2.0, None, "warning")])]);
    assert_eq!(tone_of(&rt).0, t.status_waiting_text);
    assert_eq!(tone_of(&rt).1.unwrap(), t.status_waiting().alpha(0.14));

    let rt = serve(vec![claude(vec![window("five_hour", 0.5, 2.0, None, "rejected")])]);
    assert_eq!(tone_of(&rt).0, t.error_text);
    assert_eq!(tone_of(&rt).1.unwrap(), t.error.alpha(0.14));
    assert_eq!(text_of(&rt, "usage-chip-percent"), "100%");
}

// UsageLimitsPopover.test.ts: "shows nothing without a provider"
#[test]
fn shows_nothing_without_a_provider() {
    let rt = serve(vec![]);
    assert!(info(&rt, "usage-limits-chip").is_none());
}

// UsageLimitsPopover.test.ts: "lists a bar per window with percent, reset, source and the OpenRouter credits"
#[test]
fn lists_a_bar_per_window_with_percent_reset_source_and_the_openrouter_credits() {
    let rt = open(vec![
        provider(
            "claude",
            "Claude",
            json!({ "account": "Max plan", "windows": [window("five_hour", 0.42, 2.0, None, "allowed"), window("seven_day", 0.1, 100.0, None, "allowed")] }),
        ),
        provider(
            "openrouter",
            "OpenRouter",
            json!({ "source": "OpenCode", "credits": { "remaining": 7.6, "limit": 10.0, "used": 2.4, "unit": null } }),
        ),
    ]);
    let claude = said(&rt, "claude", "home");
    assert!(claude.contains("Max plan"), "{claude}");
    assert!(claude.contains("read from Claude Code, last asked"), "{claude}");
    assert!(claude.contains("5-hour limit") && claude.contains("resets"), "{claude}");
    let bars: Vec<ElementInfo> = rt.elements().into_iter().filter(|e| e.role == "progressbar").collect();
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0].label.as_deref(), Some("42"), "aria-valuenow");
    assert!(said(&rt, "openrouter", "home").contains("$7.60 left of $10.00"));
}

// UsageLimitsPopover.test.ts: "shows GitHub's AI credits as a count, never dollars, even with no window"
#[test]
fn shows_github_s_ai_credits_as_a_count_never_dollars_even_with_no_window() {
    let rt = open(vec![provider(
        "copilot",
        "GitHub Copilot",
        json!({ "state": "no_window", "credits": { "remaining": null, "limit": null, "used": 307_846, "unit": "ai_credits" } }),
    )]);
    let text = said(&rt, "copilot", "home");
    assert!(text.contains("Pay-per-token: no limits to show."), "{text}");
    assert!(text.contains("AI credits used this month: 307,846"), "{text}");
    assert!(!text.contains('$') && !text.contains("Credits:") && !text.contains("Can't read this right now"), "{text}");
    assert_eq!(
        info(&rt, "usage-ai-credits-copilot-home").unwrap().help.as_deref(),
        Some("GitHub's count; it can run a few minutes behind.")
    );
}

// UsageLimitsPopover.test.ts: "labels the organization's pooled limit"
#[test]
fn labels_the_organization_s_pooled_limit() {
    let rt =
        open(vec![provider("copilot", "GitHub Copilot", json!({ "windows": [window("organization", 1.0, 100.0, None, "rejected")] }))]);
    let text = said(&rt, "copilot", "home");
    assert!(text.contains("Organization limit") && text.contains("100%"), "{text}");
    assert!(text_of(&rt, "usage-chip-name").contains("GitHub Copilot org"));
    assert!(!text.contains("Can't read this right now"));
}

// UsageLimitsPopover.test.ts: "explains each state that has no bars"
#[test]
fn explains_each_state_that_has_no_bars() {
    let rt = open(vec![
        provider("a", "Alpha", json!({ "state": "no_window" })),
        provider("b", "Beta", json!({ "state": "unreadable" })),
        provider("c", "Gamma", json!({ "state": "signed_out" })),
        provider("d", "Delta", json!({ "state": "unsupported", "source": null })),
    ]);
    let text = |id: &str| said(&rt, id, "home");
    assert!(text("a").contains("Pay-per-token: no limits to show"));
    assert!(text("b").contains("Can't read this right now"));
    assert!(text("c").contains("Sign in again in Claude Code"));
    assert!(text("d").contains("Fleet can't read this sign-in yet"));
    // Each state says only its own line.
    assert!(!text("a").contains("Can't read this right now"));
    assert!(!text("c").contains("Can't read this right now"));
}

// UsageLimitsPopover.test.ts: "asks before reading a provider, and Allow sends the consent"
#[test]
fn asks_before_reading_a_provider_and_allow_sends_the_consent() {
    let home = usage_fleet(&body("codex"), 204, vec![]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| {
        s.pickers.usage.limits = read_usage_limits(
            &json!({"enabled": true, "providers": [provider("codex", "Codex", json!({"state": "not_allowed", "source": "Codex CLI"}))]}),
        )
        .unwrap()
    });
    rig.rt.frame(FRAME);
    click(&mut rig.rt, "usage-limits-chip");
    rig.rt.frame(FRAME);
    let section = said(&rig.rt, "codex", "home");
    assert!(section.contains("read-only, never saves it"), "{section}");
    assert!(rig.rt.elements().iter().all(|e| e.role != "progressbar"));
    assert_eq!(text_of(&rig.rt, "usage-chip-name"), "Usage", "nothing readable: the chip still opens");
    assert!(info(&rig.rt, "usage-chip-bar").is_none() && info(&rig.rt, "usage-chip-percent").is_none());

    click(&mut rig.rt, "usage-allow-codex");
    reads(&mut rig, &home, "POST /api/usage/limits/codex/allow", 1);
}

// UsageLimitsPopover.test.ts: "asks the server to refresh (watch) while open"
#[test]
fn asks_the_server_to_refresh_watch_while_open() {
    let home = usage_fleet(&body("claude"), 204, vec![]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| s.load_usage());
    rig.until("the first read", |s| !s.pickers.usage.limits.providers.is_empty());
    assert_eq!(home.count("GET /api/usage/limits?watch=true"), 0, "closed: no watched read");
    click(&mut rig.rt, "usage-limits-chip");
    // The poll of the next frames starts the watched read.
    crate::test_wait::until_true("the watched read", || {
        rig.rt.frame(FRAME);
        home.count("GET /api/usage/limits?watch=true") >= 1
    });
    assert!(rig.rt.state.pickers.usage.watching());
    // However it shuts, the watch stops with it.
    click(&mut rig.rt, "usage-limits-scrim");
    rig.rt.frame(FRAME);
    assert!(!rig.rt.state.pickers.usage.watching());
}

// UsageLimitsPopover.test.ts: "draws a tick at an even share and the pace note only when ahead of pace"
#[test]
fn draws_a_tick_at_an_even_share_and_the_pace_note_only_when_ahead_of_pace() {
    // 3 of 5 hours gone (60%), 90% used: ahead of pace. The 7-day has no length: no tick.
    let rt =
        open(vec![claude(vec![window("five_hour", 0.9, 2.0, Some(18_000), "warning"), window("seven_day", 0.1, 80.0, None, "allowed")])]);
    let ticks: Vec<ElementInfo> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.ends_with("-tick"))).collect();
    assert_eq!(ticks.len(), 1);
    let title = ticks[0].help.clone().unwrap_or_default();
    assert!(title.starts_with("An even share of this window by now: 6"), "{title}");
    let notes: Vec<ElementInfo> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.ends_with("-pace"))).collect();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].text.clone().unwrap_or_default().contains("Ahead of pace"));
    let _ = at;
}

// UsageLimitsPopover.test.ts: "shows a tick but no note when under pace"
#[test]
fn shows_a_tick_but_no_note_when_under_pace() {
    let rt = open(vec![claude(vec![window("five_hour", 0.2, 2.0, Some(18_000), "allowed")])]);
    assert_eq!(rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.ends_with("-tick"))).count(), 1);
    assert!(rt.elements().iter().all(|e| !e.key.as_deref().is_some_and(|k| k.ends_with("-pace"))));
}

// UsageLimitsPopover.test.ts: "shows no machine headings with one machine"
#[test]
fn shows_no_machine_headings_with_one_machine() {
    let rt = open(vec![claude(vec![window("five_hour", 0.4, 1.0, None, "allowed")])]);
    assert!(rt.elements().iter().all(|e| !e.key.as_deref().is_some_and(|k| k.starts_with("usage-machine-"))));
}

fn two_machines() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_usage(crate::mock::usage::Case::Machines, true));
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt
}

// UsageLimitsPopover.test.ts: "two machines groups by connected machine, shows a shared account once with also-on, and asks the other lazily"
#[test]
fn two_machines_groups_by_connected_machine_shows_a_shared_account_once_with_also_on_and_asks_the_other_lazily() {
    let rt = two_machines();
    let headings: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("usage-machine-")))
        .filter_map(|e| e.text)
        .collect();
    // Falcon answered; harbor is away and lantern hasn't answered: neither is a group.
    assert_eq!(headings, ["ATLAS", "FALCON"]);
    assert_eq!(rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("usage-name-codex"))).count(), 1);
    assert!(text_of(&rt, "usage-also-on-codex-home").contains("also on falcon"));
    assert!(info(&rt, "usage-name-openrouter-m-falcon").is_some());
    // The other machine is asked only while the popover is open (`asks_the_other_machines_only_while_watched…`, piece I's).
    assert_eq!(rt.state.usage_other_machines().len(), 1);
}

// UsageLimitsPopover.test.ts: "two machines sends Allow only for the live machine's providers"
#[test]
fn two_machines_sends_allow_only_for_the_live_machine_s_providers() {
    let mut rt = two_machines();
    rt.update(|s, _| {
        let away = read_usage_limits(
            &json!({"enabled": true, "providers": [provider("openrouter", "OpenRouter", json!({"state": "not_allowed"}))]}),
        )
        .unwrap();
        s.pickers.usage.others.insert("m-falcon".into(), away.providers);
        s.pickers.usage.limits =
            read_usage_limits(&json!({"enabled": true, "providers": [provider("claude", "Claude", json!({}))]})).unwrap();
    });
    rt.frame(FRAME);
    assert!(info(&rt, "usage-allow-openrouter").is_none());
    assert!(said(&rt, "openrouter", "m-falcon").contains("Open falcon to allow it there."));
}

// StatusBar.test.ts: "shows the usage chip unless the Settings switch hides it"
#[test]
fn shows_the_usage_chip_unless_the_settings_switch_hides_it() {
    let mut rt = serve(vec![claude(vec![window("five_hour", 0.4, 1.0, None, "allowed")])]);
    assert!(info(&rt, "usage-limits-chip").is_some());
    rt.update(|s, _| {
        s.pickers.prefs.set(crate::fleet::usage::CHIP_PREFERENCE_KEY, "false");
    });
    rt.frame(FRAME);
    assert!(info(&rt, "usage-limits-chip").is_none());
}

#[test]
fn a_press_outside_shuts_the_popover_and_the_chip_toggles_it() {
    let mut rt = open(vec![claude(vec![window("five_hour", 0.4, 1.0, None, "allowed")])]);
    assert_eq!(rt.state.ui.open, Some(Open::Popover("usage")));
    assert_eq!(info(&rt, "usage-limits-popover").unwrap().label.as_deref(), Some("Usage limits"));
    click(&mut rt, "usage-limits-scrim");
    assert_eq!(rt.state.ui.open, None);
}
