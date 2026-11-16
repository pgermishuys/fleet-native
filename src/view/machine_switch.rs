//! Ports `layout/MachineSwitch.vue`: "<Label> for" in a view's menu header, the chip that says which machine the view's
//! detail pane shows and the menu that changes it (rows 17.51-17.53). The view owns a `MachineChoice` in `State` and
//! says where it is with a `Switch`; the chip is drawn only when another machine is listed.
use crate::State;
use crate::fleet::machine_choice::MachineChoice;
use crate::icons::{CHECK, CHEVRON_DOWN};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::overlay;
use rune::{Align, Cursor, El, Side, col, el, icon, row, text};

/// One view's switch: its words, its key prefix (`testid`) and where its choice lives.
#[derive(Clone, Copy)]
pub struct Switch {
    /// "Settings": the chip's label is "<label> for machine", the menu's heading "<label> for".
    pub label: &'static str,
    pub testid: &'static str,
    pub choice: fn(&mut State) -> &mut MachineChoice,
}

impl Switch {
    pub fn chip_key(&self) -> String {
        format!("{}-machine-switch", self.testid)
    }
    pub fn row_key(&self, machine: &str) -> String {
        format!("{}-machine-{machine}", self.testid)
    }
    fn menu_key(&self) -> String {
        format!("{}-machine-menu", self.testid)
    }
    fn scrim_key(&self) -> String {
        format!("{}-machine-scrim", self.testid)
    }
}

/// `.mx-dot`: 7px, `--running`.
fn dot(t: &Theme) -> El {
    el().size(7.0, 7.0).radius(999.0).fill(t.running)
}

/// "N session(s)" as Fleet words it: home's from its list, another's from its last read.
pub fn count_words(n: usize) -> String {
    format!("{n} {}", if n == 1 { "session" } else { "sessions" })
}

/// `.mx-switch`: nothing when only home is listed (`enabled`).
pub fn chip(s: &State, t: &Theme, sw: Switch, choice: &MachineChoice) -> Option<El> {
    if !choice.enabled(&s.machines) {
        return None;
    }
    let name = choice.entry(&s.machines).name;
    let washed = choice.open;
    let mut chip = row()
        .key(sw.chip_key())
        .role("button")
        .label(format!("{} for machine", sw.label))
        .gap(6.0)
        .center_y()
        .pl(8.0)
        .pr(7.0)
        .py(3.0)
        .radius(6.0)
        .border(1.0, t.border)
        .shrink()
        .min_w(0.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .focus_ring(2.0, 2.0, t.accent)
        .child(dot(t))
        .child(text(name).font(mono(11.5, 400.0, 1.5)).color(t.coral_text).truncate().shrink())
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press_tx(move |s: &mut State, tx| {
            let c = (sw.choice)(s);
            c.open = !c.open;
            if c.open {
                tx.focus(sw.menu_key(), false);
            }
        });
    if washed {
        chip = chip.fill(t.wash(6.0));
    }
    Some(chip)
}

/// How many sessions `key`'s machine has: home's list, or what another was last read to hold (`countOf`).
fn count_of(s: &State, key: &str) -> usize {
    if key == crate::fleet::settings::HOME_MACHINE_KEY {
        s.projects.iter().map(|p| p.sessions.len()).sum()
    } else {
        s.machines.others.get(key).map_or(0, |m| m.sessions.len())
    }
}

/// `.mx-menu`: the heading and a row per machine, home first. Choosing closes it.
pub fn overlay(s: &State, t: &Theme, sw: Switch, choice: &MachineChoice) -> Vec<El> {
    if !choice.open || !choice.enabled(&s.machines) {
        return Vec::new();
    }
    let shown = choice.key(&s.machines).to_string();
    let mut rows = vec![
        row()
            .px(8.0)
            .pt(6.0)
            .pb(4.0)
            .child(text(format!("{} for", sw.label).to_uppercase()).font(sans(10.0, 400.0, 1.3).tracking(0.6)).color(t.muted)),
    ];
    for entry in s.machines.entries() {
        let key = entry.key.clone();
        let n = count_of(s, &entry.key);
        let mut r = row()
            .key(sw.row_key(&entry.key))
            .role("menuitem")
            .label(entry.name.clone())
            .w_full()
            .px(8.0)
            .py(7.0)
            .gap(8.0)
            .center_y()
            .radius(RADIUS_BTN - 2.0)
            .cursor(Cursor::Default)
            .hover(t.wash(7.0))
            .child(dot(t))
            .child(
                row()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(4.0)
                    .center_y()
                    .child(text(entry.name.clone()).font(sans(13.0, 400.0, 1.5)).color(t.text).truncate().shrink())
                    .child(text(count_words(n)).font(sans(11.0, 400.0, 1.3)).color(t.muted)),
            );
        if key == shown {
            r = r.child(icon(&CHECK, 12.0).color(t.accent_text).label("Shown"));
        }
        rows.push(r.on_press_tx(move |s: &mut State, tx| {
            (sw.choice)(s).pick(&key);
            (sw.choice)(s).open = false;
            tx.focus(sw.chip_key(), false);
        }));
    }
    let panel = col()
        .key(sw.menu_key())
        .role("menu")
        .focus_group()
        // Fleet draws it 250 wide with its text 22px in: the popover's own padding wins over `.mx-menu`'s 4px (measured on the live client).
        .min_w(250.0)
        .padding(14.0)
        .radius(crate::theme::RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .items_stretch()
        .box_shadow(t.pop_shadow())
        .children(rows)
        .modal(true, move |s: &mut State| (sw.choice)(s).open = false);
    vec![
        overlay::scrim(&sw.scrim_key(), move |s: &mut State| (sw.choice)(s).open = false),
        overlay::anchored(panel, &sw.chip_key(), Side::Bottom, Align::Start, 4.0),
    ]
}

/// The scene's one switch: Settings' own, at the top of its menu panel (`SettingsNavPanel.vue`: header `14px 16px 10px`,
/// switch under it in `0 12px 6px`). Component 16 replaces it with the real menu and keeps `Switch`.
pub const SCENE: Switch =
    Switch { label: "Settings", testid: "settings", choice: |s| s.machine_switch_scene.as_mut().expect("the scene sets it") };

pub fn scene_page(s: &State, t: &Theme) -> El {
    let choice = s.machine_switch_scene.clone().unwrap_or_default();
    let header = row()
        .pt(4.0)
        .child(row().px(16.0).pt(14.0).pb(10.0).child(text("SETTINGS").font(sans(10.0, 600.0, 1.3).tracking(0.5)).color(t.muted)));
    let mut panel = col().w(crate::view::SIDEBAR_W).h_full().fill(t.panel_bg).border(1.0, t.border).radius(12.0).child(header);
    if let Some(chip) = chip(s, t, SCENE, &choice) {
        panel = panel.child(row().px(12.0).pb(6.0).child(chip));
    }
    row().grow(1.0).h_full().child(panel)
}
