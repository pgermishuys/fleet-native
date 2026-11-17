//! Settings → What agents can do › Browser (`BrowserSection.vue`): whether agents get a browser of their own, which pages it may
//! open, whether page scripts run, and two rows that aren't built yet. Sizes are Fleet's scoped CSS px (`.browser-*`); the
//! switch is the section's own 36 x 20 one, not `SettingSwitch`'s.
use super::kit;
use crate::State;
use crate::fleet::settings::browser::{Change, Pages};
use crate::icons::{CIRCLE_ALERT, INFO};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::confirm::spinner;
use rune::{Color, Cursor, El, col, el, icon, row, text};

/// `.browser-switch`: a 36 x 20 track, a 16px white knob that travels 16. Disabled it is 60% and doesn't press.
fn switch(t: &Theme, key: &str, label: &str, on: bool, disabled: bool, press: impl Fn(&mut State) + 'static) -> El {
    let (w, h, knob) = (36.0, 20.0, 16.0);
    let mut track = row()
        .key(key.to_string())
        .role("switch")
        .label(label.to_string())
        .checked(on)
        .w(w)
        .h(h)
        .px(2.0)
        .center_y()
        .radius(h / 2.0)
        .fill(if on { t.accent } else { t.border })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .child(el().size(knob, knob).radius(knob / 2.0).fill(Color::WHITE).ml(if on { w - knob - 4.0 } else { 0.0 }));
    track = if disabled { track.opacity(0.6).disabled(true).cursor(Cursor::Default) } else { track.on_press(press) };
    track
}

/// A row's two lines: the bold one (13, 500) and the small one under it (12, muted).
fn words(t: &Theme, title: &str, small: &str, faded: bool) -> El {
    let c = col()
        .gap(2.0)
        .grow(1.0)
        .min_w(0.0)
        .child(text(title.to_string()).font(sans(13.0, 500.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0))
        .child(text(small.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0));
    if faded { c.opacity(0.6) } else { c }
}

/// `.browser-row`: the words and a control side by side; every row but the last has a rule under it.
fn line(t: &Theme, last: bool, children: Vec<El>) -> El {
    let mut r = row().w_full().center_y().gap(16.0).px(16.0).py(13.0);
    if !last {
        r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    }
    for c in children {
        r = r.child(c);
    }
    r
}

/// A radio drawn as a ring with a dot: Rune has no native one. The accent shows when it is picked.
fn radio(t: &Theme, on: bool) -> El {
    let ring = el().size(13.0, 13.0).radius(6.5).border(1.0, if on { t.accent } else { t.muted }).border_layout().center();
    if on { ring.child(el().size(7.0, 7.0).radius(3.5).fill(t.accent)) } else { ring }
}

fn pages_row(s: &State, t: &Theme, enabled: bool, busy: bool) -> El {
    let now = s.browser_copy().settings.pages;
    let mut group = col().key("browser-pages-group").role("radiogroup").label("Pages agents can open").w_full().gap(6.0);
    if !enabled {
        group = group.opacity(0.6);
    }
    for p in Pages::ALL {
        let on = p == now;
        let off = busy || !enabled;
        let mut r = row()
            .key(format!("browser-pages-{}", p.value()))
            .role("radio")
            .label(p.label().to_string())
            .checked(on)
            .w_full()
            .items_start()
            .gap(9.0)
            .focusable(true)
            .focus_ring(2.0, 2.0, t.accent)
            .cursor(if off { Cursor::Default } else { Cursor::Pointer })
            .child(radio(t, on).mt(2.0))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .child(text(p.label().to_string()).font(sans(12.5, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0))
                    .child(text(p.description().to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0)),
            );
        r = if off { r.disabled(true) } else { r.on_press(move |s: &mut State| s.save_browser(Change::Pages(p))) };
        group = group.child(r);
    }
    col()
        .w_full()
        .gap(8.0)
        .px(16.0)
        .py(13.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(words(
            t,
            "Pages agents can open",
            "Fleet checks every address before the agent's tab goes there, links it clicks included.",
            !enabled,
        ))
        .child(group)
}

/// `ErrorNote` (not compact): tinted, bordered, with a 16px alert icon.
fn error_note(t: &Theme, message: &str) -> El {
    row()
        .key("browser-error")
        .role("alert")
        .w_full()
        .mt(10.5)
        .gap(7.0)
        .items_start()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_CARD)
        .fill(t.error.alpha(0.1))
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text))
        .child(text(message.to_string()).font(sans(12.25, 400.0, 1.5)).color(t.error_text).wrap().shrink().min_w(0.0))
}

fn later(t: &Theme) -> El {
    row()
        .px(8.0)
        .py(2.0)
        .radius(999.0)
        .fill(t.text.alpha(0.07))
        .child(text("Later".to_string()).font(sans(11.0, 400.0, 1.5)).color(t.muted))
}

pub fn section(s: &State, t: &Theme) -> El {
    let copy = s.browser_copy();
    let busy = copy.loading || copy.saving;
    let enabled = copy.settings.enabled;
    let mut head = row().w_full().items_start().gap(14.0).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(3.5)
            .child(text("Browser".to_string()).font(sans(12.25, 600.0, 1.5)).color(t.text))
            .child(
                text(
                    "Agents can open their own tab on a page, read it, click and type, to try what they built. They use Fleet's headless browser, not yours, so your view and your sign-ins don't change."
                        .to_string(),
                )
                .font(sans(10.5, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .shrink()
                .min_w(0.0),
            ),
    );
    if busy {
        head = head.child(spinner(t.muted).mt(3.5));
    }
    let on_switch =
        switch(t, "browser-enabled-switch", "Agents can use a browser", enabled, busy, move |s| s.save_browser(Change::Enabled(!enabled)));
    let scripts = copy.settings.scripts;
    let scripts_switch = switch(t, "browser-scripts-switch", "Run scripts in pages", scripts, busy || !enabled, move |s| {
        s.save_browser(Change::Scripts(!scripts))
    });
    let upload = switch(t, "browser-upload-switch", "Upload files to pages", false, true, |_| {});
    let rows = col()
        .key("browser-settings")
        .w_full()
        .mt(17.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .clip_rounded(RADIUS_CARD)
        .child(line(
            t,
            false,
            vec![
                words(
                    t,
                    "Agents can use a browser",
                    "Adds browser tools to sessions that can have them (each harness's tab says which). Off: OpenCode 2 doesn't see its browser tools, Claude Code and GitHub Copilot don't see Fleet's from their next message, and OpenCode's answer that the browser is off.",
                    false,
                ),
                on_switch,
            ],
        ))
        .child(pages_row(s, t, enabled, busy))
        .child(line(
            t,
            false,
            vec![
                words(
                    t,
                    "Run scripts in pages",
                    "Lets the agent run its own JavaScript in a page. Useful for checks, but a script can read the page's storage and call other local services.",
                    !enabled,
                ),
                scripts_switch,
            ],
        ))
        .child(line(
            t,
            false,
            vec![
                words(
                    t,
                    "Upload files to pages",
                    "Not in this version. OpenCode 2 reads the file itself and Fleet only sees its name, so Fleet can't keep uploads inside the session's folder.",
                    true,
                ),
                upload,
            ],
        ))
        .child(line(
            t,
            true,
            vec![
                words(
                    t,
                    "Performance traces and memory snapshots",
                    "Later. OpenCode 2 offers them; Fleet answers \"not supported\" until they're built.",
                    true,
                ),
                later(t),
            ],
        ));
    let fine = row()
        .key("browser-fine")
        .w_full()
        .mt(17.5)
        .gap(8.0)
        .items_start()
        .child(icon(&INFO, 13.0).color(t.accent).mt(2.0))
        .child(
            text(
                "The agent's tabs share the browser Fleet uses for screenshots: about 240 MB while a tab is open. They close after 15 minutes without use, and the browser quits two minutes after its last use."
                    .to_string(),
            )
            .font(sans(12.0, 400.0, 1.5))
            .color(t.muted)
            .wrap()
            .shrink()
            .min_w(0.0),
        );
    let mut card = col()
        .key("browser-section")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .px(14.0)
        .py(14.0)
        .child(head);
    if let Some(why) = copy.error {
        card = card.child(error_note(t, &why));
    }
    kit::anchor("browser", card.child(rows).child(fine))
}
