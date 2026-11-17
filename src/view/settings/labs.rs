//! Settings → Labs (`LabsSection.vue`): one group, "Experiments", of switches that are each off until turned on. Every one is a
//! server preference on this machine. A row says what it adds while it is on, with an Open link to the place it adds.
use super::kit::{self, Row, SwitchState};
use crate::fleet::settings::switches::{self, Switch};
use crate::theme::{Theme, sans};
use crate::ui::rem;
use crate::{Rail, State};
use rune::{El, col, row, text};

struct Spec {
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    more: &'static str,
    key: &'static str,
    label: &'static str,
    switch: Switch,
    adds: &'static str,
}

const JOBS: Spec = Spec {
    id: "jobs",
    title: "Jobs",
    summary: "One queue; Fleet starts each job wherever a seat is free.",
    more: "One queue of jobs. Fleet starts each on whichever of your machines has a free seat. Give machines seats in Settings → \
           Machines.",
    key: "jobs-switch",
    label: "Enable Jobs",
    switch: switches::JOBS,
    adds: "Jobs in the rail, seats on each machine",
};
const MISSION_CONTROL: Spec = Spec {
    id: "mission-control",
    title: "Mission control",
    summary: "One screen for every running agent. It becomes the first screen.",
    more: "One screen for every running agent on every machine: what needs you, what each is doing right now, and a nudge box. It \
           becomes the first screen when Fleet opens.",
    key: "mission-control-switch",
    label: "Enable Mission control",
    switch: switches::MISSION_CONTROL,
    adds: "Fleet in the rail, the first screen",
};
const MEMORY_SEARCH: Spec = Spec {
    id: "memory-search",
    title: "Search older notes",
    summary: "Sessions read a short list of notes and search the rest.",
    more: "Every session reads 15 notes about its repository and 8 about the machine, and finds the rest with a search tool when it \
           starts on a new area. A new note goes in the short list and the least used one moves down to search. Nothing is deleted, \
           and turning this off puts every note back in the prompt. A weaker model may never search, so this stays here until it has \
           been checked on every harness. Needs Memory on.",
    key: "memory-search-switch",
    label: "Enable searching older notes",
    switch: switches::MEMORY_SEARCH,
    adds: "Two groups in Memory",
};
const WORKFLOWS: Spec = Spec {
    id: "workflows",
    title: "Workflows",
    summary: "Workflows in the rail; Fleet runs their steps and stops where one asks you.",
    more: "Show Workflows in the rail, and let Fleet run workflows: it moves between steps and stops where a step asks you. Off: no \
           runner, no step tool, nothing changes.",
    key: "workflows-switch",
    label: "Enable workflows",
    switch: switches::WORKFLOWS,
    adds: "Workflows in the rail",
};

/// A row whose switch is on `spec.switch`. `adds_on` is whether the adds line shows; `locked` disables a switch that is off
/// (Search older notes while Memory is off), so one that is already on can still be turned off.
fn labs_row(s: &State, t: &Theme, spec: &Spec, summary: &str, adds_on: bool, locked: bool, open: impl Fn(&mut State) + 'static) -> El {
    let on = spec.switch.enabled(&s.pickers.prefs);
    let saving = s.setting_saving(spec.switch.key);
    let state = SwitchState { saving, disabled: s.preferences_loading() || saving || locked, pending: false };
    let switch = spec.switch;
    let mut r = Row::new(spec.id, spec.title).summary(summary).more(spec.more);
    if adds_on {
        r = r.adds(spec.adds, Some(open));
    }
    r.control(kit::setting_switch(t, spec.key, spec.label, on, state, move |s: &mut State| s.toggle_setting(switch))).build(s, t)
}

/// Rail entries are routes in Fleet (`router.navigate`): opening one changes the rail and leaves the switch alone.
fn go(rail: Rail) -> impl Fn(&mut State) {
    move |s: &mut State| s.rail = rail
}

/// Keep every machine live is drawn and saved only: this app has no live feeds yet (owner's decision 6), so the row says so.
fn live_note(t: &Theme) -> El {
    row().key("live-machines-later").mt(7.0).child(
        text("This app doesn't keep live feeds yet. The choice is kept for Fleet on your phone and in the browser.")
            .font(sans(rem(0.75), 400.0, 1.33))
            .color(t.muted)
            .wrap()
            .shrink()
            .min_w(0.0),
    )
}

pub fn content(s: &State, t: &Theme) -> El {
    let prefs = &s.pickers.prefs;
    let on = |sw: Switch| sw.enabled(prefs);
    let memory_on = on(switches::MEMORY);
    let search_on = on(switches::MEMORY_SEARCH);
    let mut rows = Vec::new();
    // Only once another machine is known (`machines.hasMachines`).
    if s.machines.has_machines() {
        let live = switches::LIVE_MACHINES;
        let saving = s.setting_saving(live.key);
        let state = SwitchState { saving, disabled: s.preferences_loading() || saving, pending: false };
        rows.push(
            Row::new("live-machines", "Keep every machine live")
                .summary("Other machines update as they change, not every 15 seconds.")
                .more("Other machines' sessions update as they change, instead of every 15 seconds, and open here.")
                .below(live_note(t))
                .control(kit::setting_switch(
                    t,
                    "live-machines-switch",
                    "Keep every machine live",
                    on(live),
                    state,
                    move |s: &mut State| s.toggle_setting(live),
                ))
                .build(s, t),
        );
    }
    rows.push(labs_row(s, t, &JOBS, JOBS.summary, on(JOBS.switch), false, go(Rail::Jobs)));
    rows.push(labs_row(s, t, &MISSION_CONTROL, MISSION_CONTROL.summary, on(MISSION_CONTROL.switch), false, go(Rail::Fleet)));
    let search_summary = if memory_on { MEMORY_SEARCH.summary.to_string() } else { format!("Needs Memory on. {}", MEMORY_SEARCH.summary) };
    rows.push(labs_row(s, t, &MEMORY_SEARCH, &search_summary, search_on && memory_on, !memory_on && !search_on, |s: &mut State| {
        s.open_settings("memory", None)
    }));
    rows.push(labs_row(s, t, &WORKFLOWS, WORKFLOWS.summary, on(WORKFLOWS.switch), false, go(Rail::Workflows)));
    col()
        .key("labs-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Labs))
        .child(kit::group(t, "experiments", "Experiments", Some("Each is off until you turn it on"), rows))
}
