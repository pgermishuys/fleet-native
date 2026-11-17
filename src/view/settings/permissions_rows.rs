//! Settings → What agents can do, the rows after Run commands (`PermissionsSection.vue`): Message other sessions, Send work to
//! other machines, When nobody is watching, and the fine print. Sizes are Fleet's scoped CSS px (`.reach-*`).
use super::kit;
use super::permissions::{head, small};
use crate::State;
use crate::fleet::machines::AgentWork;
use crate::fleet::settings::permissions_more::{Unattended, work_label};
use crate::icons::{CHEVRON_DOWN, INFO};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::{Open, select};
use rune::{Cursor, El, Span, col, icon, rich, row, text};
use std::rc::Rc;

const MESSAGES: [(&str, &str); 2] = [("allowed", "Allowed"), ("off", "Off")];
const WORK: [AgentWork; 3] = [AgentWork::Ask, AgentWork::Always, AgentWork::Never];

fn note(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.5, 400.0, 1.45)).color(t.muted).wrap().shrink().min_w(0.0)
}

fn section(key: &str, children: Vec<El>) -> El {
    let mut r = col().key(key.to_string()).w_full().gap(8.0).px(16.0).py(14.0);
    for c in children {
        r = r.child(c);
    }
    r
}

fn summary(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// Row 3: whether sessions may message each other. The summary holds `fleet_message` as inline code.
pub fn messages(s: &State, t: &Theme) -> El {
    let (a, code, b) = (
        "An agent asks or tells another session something with ",
        "fleet_message",
        ". The message says which session sent it, and the agent there treats it as a teammate's request, not yours.",
    );
    let words = format!("{a}{code}{b}");
    let span = Span::new(a.len(), a.len() + code.len()).mono().size(11.5).bg(t.main_bg, 4.0);
    let body = rich(words, vec![span]).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap().shrink().min_w(0.0);
    let segments = kit::segments(
        t,
        "reach-messages",
        "Message other sessions",
        &MESSAGES,
        if s.permission_messages() { "allowed" } else { "off" },
        s.harness_prefs_loading() || s.settings.permissions.messages_saving,
        Rc::new(|s: &mut State, v: &str| s.set_permission_messages(v == "allowed")),
    );
    section(
        "reach-row-messages",
        vec![
            head(t, "messages", "Message other sessions", &s.messages_tag(), body, segments),
            note(t, "Applies to sessions started afterwards."),
        ],
    )
}

/// `.reach-chip` for a machine: its name, its value's trigger and the level it runs at.
fn machine_chip(t: &Theme, key: &str, name: &str, work: AgentWork, level: &str) -> El {
    let id = format!("reach-machine-{key}");
    let machine = key.to_string();
    let trigger = row()
        .key(format!("{id}-select"))
        .role("combobox")
        .label(format!("{name}: agents here can send it work"))
        .center_y()
        .gap(3.0)
        .px(2.0)
        .py(1.0)
        .radius(999.0)
        .cursor(Cursor::Pointer)
        .hover(t.text.alpha(0.07))
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(small(work_label(work), 12.0, 600.0, t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press(move |s: &mut State| {
            s.settings.permissions.machine_chip = Some(machine.clone());
            s.ui.open = Some(Open::Select("permission-machine"));
        });
    row()
        .key(id)
        .center_y()
        .gap(6.0)
        .pl(10.0)
        .pr(4.0)
        .py(2.0)
        .radius(999.0)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(small(name, 12.0, 500.0, t.text))
        .child(trigger)
        .child(row().pr(6.0).child(small(&format!("\u{b7} runs at {level}"), 12.0, 400.0, t.muted)))
}

fn machine_add(t: &Theme) -> El {
    row()
        .key("reach-add-machines")
        .role("combobox")
        .label("Add an exception to Send work to other machines")
        .center_y()
        .gap(4.0)
        .pl(10.0)
        .pr(6.0)
        .py(2.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .cursor(Cursor::Pointer)
        .hover(t.text.alpha(0.07))
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(small("+ exception", 12.0, 400.0, t.muted))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press(|s: &mut State| {
            s.settings.permissions.machine_add = true;
            s.ui.open = Some(Open::Select("permission-machine-add"));
        })
}

/// "No other machines yet." and the way to add one: the link is underlined, as Fleet's always is.
fn no_machines(t: &Theme) -> El {
    let link = "Add one in Machines";
    let button = row()
        .key("reach-add-in-machines")
        .role("button")
        .label(link)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(
            rich(link.to_string(), vec![Span::new(0, link.len()).underline(t.accent_text)])
                .font(sans(11.5, 400.0, 1.45))
                .color(t.accent_text),
        )
        .on_press(|s: &mut State| s.open_settings("machines", None));
    row().wrap().gap(4.0).child(note(t, "No other machines yet.")).child(button)
}

/// Row 4: work sent to other machines. Every machine starts at Ask; the exceptions are the ones set to Always or Never.
pub fn machines(s: &State, t: &Theme) -> El {
    let fixed = row()
        .key("reach-machines-fixed")
        .px(11.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .help("Every machine starts at Ask. Set one to Always or Never below.")
        .child(text("Ask".to_string()).font(sans(12.5, 600.0, 1.5)).color(t.text));
    let words = "Start a session on another machine and give it a task. The agent asks in the conversation first, and the work runs under that machine's own level.";
    let mut kids = vec![head(t, "machines", "Send work to other machines", "default", summary(t, words), fixed)];
    if !s.permission_machine_is_live() {
        let live = s.live_machine().name;
        kids.push(
            note(
                t,
                &format!(
                    "Which machines {} may send work to isn't shown here yet. {live}'s list is on its own page.",
                    s.permission_shown_name()
                ),
            )
            .key("reach-machines-live-only"),
        );
    } else if s.permission_machines_listed() {
        let mut chips = row().wrap().center_y().gap(6.0);
        for (entry, work) in s.permission_machine_exceptions() {
            chips = chips.child(machine_chip(t, &entry.key, &entry.name, work, s.permission_machine_level(&entry)));
        }
        if !s.permission_machine_candidates().is_empty() {
            chips = chips.child(machine_add(t));
        }
        kids.push(chips);
    } else {
        kids.push(no_machines(t));
    }
    if let Some(why) = s.permission_machine_error() {
        kids.push(kit::error_note(t, "reach-machines-error", &why));
    }
    section("reach-row-machines", kids)
}

/// The open value list of a machine's chip: Ask, Always, Never.
pub fn machine_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(key) = s.settings.permissions.machine_chip.clone() else { return Vec::new() };
    let options: Vec<(&str, &str)> = WORK.iter().map(|w| (crate::fleet::machine_actions::agent_work_word(*w), work_label(*w))).collect();
    let current = s.machines.agent_work.get(&key).map(|w| crate::fleet::machine_actions::agent_work_word(*w));
    let anchor = format!("reach-machine-{key}-select");
    let choose: select::Choose = Rc::new(move |s: &mut State, v: &str| {
        if let Some(w) = crate::fleet::machine_actions::parse_agent_work(v) {
            s.set_permission_machine(&key, w);
        }
    });
    select::content(t, "reach-machine-list", &anchor, 140.0, &options, current, choose)
}

/// The machines' "+ exception" list: each machine at Ask, once as Always and once as Never.
pub fn machine_add_list(s: &State, t: &Theme) -> Vec<El> {
    let words: Vec<(String, String)> = s
        .permission_machine_candidates()
        .iter()
        .flat_map(|e| {
            [AgentWork::Always, AgentWork::Never].map(|w| {
                (format!("{}|{}", e.key, crate::fleet::machine_actions::agent_work_word(w)), format!("{} \u{b7} {}", e.name, work_label(w)))
            })
        })
        .collect();
    let options: Vec<(&str, &str)> = words.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let choose: select::Choose = Rc::new(|s: &mut State, v: &str| s.add_permission_machine(v));
    select::content(t, "reach-machine-add-list", "reach-add-machines", 200.0, &options, None, choose)
}

/// Row 5: what a run does at an ask nobody is there to answer. The note under the summary follows the choice.
pub fn unattended(s: &State, t: &Theme) -> El {
    let now = s.permission_unattended();
    let options: Vec<(&str, &str)> = Unattended::ALL.iter().map(|u| (u.value(), u.label())).collect();
    let segments = kit::segments(
        t,
        "reach-unattended",
        "When nobody is watching",
        &options,
        now.value(),
        false,
        Rc::new(|s: &mut State, v: &str| s.set_permission_unattended(Unattended::parse(Some(v)))),
    );
    let words = format!("Automations and workflow steps Fleet finishes can't answer an ask. {}", now.note());
    section(
        "reach-row-unattended",
        vec![head(t, "unattended", "When nobody is watching", &s.unattended_tag(), summary(t, &words), segments)],
    )
}

/// `.reach-fine`: what "Don't ask again" covers.
pub fn fine_print(t: &Theme) -> El {
    let (a, b) =
        ("Don't ask again", " lasts for the rest of the session, subagents included. A subagent's asks show on the session it works for.");
    let span = Span::new(0, a.len()).weight(600.0).color(t.text);
    row()
        .key("reach-fine")
        .w_full()
        .gap(8.0)
        .items_start()
        .child(icon(&INFO, 13.0).color(t.accent).mt(3.0))
        .child(rich(format!("{a}{b}"), vec![span]).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0))
}
