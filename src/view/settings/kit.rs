//! The setting primitives every Settings page is built from (`SettingGroup.vue`, `SettingRow.vue`, `SettingSwitch.vue`,
//! `SettingSegments.vue`, `ui/error-note/ErrorNote.vue` at `compact`). Every id is stable, so an anchor, a link or a test
//! reaches a group (`settings-group-<id>`) or a row (`settings-row-<id>`).
use crate::State;
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::{confirm::spinner, rem};
use rune::{Color, Span};
use rune::{Cursor, El, Key, KeyEvent, col, el, icon, rich, row, text};
use std::rc::Rc;

/// `SettingGroup`: the title (and a hint on the right) over one bordered card of rows divided by rules.
pub fn group(t: &Theme, id: &str, title: &str, hint: Option<&str>, rows: Vec<El>) -> El {
    let mut head = row()
        .w_full()
        .items_end()
        .justify_between()
        .gap(10.5)
        .px(3.5)
        .mb(7.0)
        .child(text(title.to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text));
    if let Some(hint) = hint {
        head = head.child(text(hint.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    let mut body = col().w_full().radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.card_bg).clip_rounded(RADIUS_CARD);
    for (at, r) in rows.into_iter().enumerate() {
        body = body.child(if at == 0 { r } else { col().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(r) });
    }
    col().key(format!("settings-group-{id}")).w_full().child(head).child(body)
}

/// A mark an anchor can scroll to without being a group or a row (`data-settings-anchor`): Folders' Storage, say.
pub fn anchor(id: &str, inner: El) -> El {
    col().key(format!("settings-anchor-{id}")).w_full().child(inner)
}

type Open = Rc<dyn Fn(&mut State)>;
/// What a segment does when it is picked: the choice's value.
pub type Pick = Rc<dyn Fn(&mut State, &str)>;

/// One setting. Build it with [`Row::new`], add what it has, and call [`Row::build`].
pub struct Row {
    id: String,
    title: String,
    summary: Option<String>,
    badge: Option<El>,
    more: Option<String>,
    adds: Option<String>,
    open: Option<Open>,
    below: Option<El>,
    control: Option<El>,
}

impl Row {
    pub fn new(id: &str, title: &str) -> Row {
        Row {
            id: id.to_string(),
            title: title.to_string(),
            summary: None,
            badge: None,
            more: None,
            adds: None,
            open: None,
            below: None,
            control: None,
        }
    }
    /// The one-line summary under the title.
    pub fn summary(mut self, words: &str) -> Row {
        self.summary = Some(words.to_string());
        self
    }
    /// A chip after the title (`#badge`).
    pub fn badge(mut self, badge: El) -> Row {
        self.badge = Some(badge);
        self
    }
    /// The full text a More link opens (`#more`).
    pub fn more(mut self, words: &str) -> Row {
        self.more = Some(words.to_string());
        self
    }
    /// What a Labs switch gives you while it is on; `open` is where its Open link goes, none for `noOpen`.
    pub fn adds(mut self, words: &str, open: Option<impl Fn(&mut State) + 'static>) -> Row {
        self.adds = Some(words.to_string());
        self.open = open.map(|f| Rc::new(f) as Open);
        self
    }
    /// Under everything in the left column: cards, error notes (`#below`).
    pub fn below(mut self, below: El) -> Row {
        self.below = Some(below);
        self
    }
    /// The control on the right (`#control`).
    pub fn control(mut self, control: El) -> Row {
        self.control = Some(control);
        self
    }

    pub fn build(self, s: &State, t: &Theme) -> El {
        let id = self.id;
        let mut left = col().grow(1.0).shrink().min_w(0.0);
        let mut head = row().items_center().gap(7.0).child(text(self.title).font(sans(rem(0.875), 500.0, 1.43)).color(t.text));
        if let Some(badge) = self.badge {
            head = head.child(badge);
        }
        left = left.child(head);
        let open = self.more.is_some() && s.settings.more_open.contains(&id);
        if self.summary.is_some() || self.more.is_some() {
            let mut line = row().w_full().flex_wrap().items_end().mt(2.0);
            if let Some(words) = self.summary {
                line = line.child(text(words).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0));
            }
            if self.more.is_some() {
                let toggled = id.clone();
                line = line.child(
                    row()
                        .key(format!("setting-more-{id}"))
                        .role("button")
                        .label(if open { "Less" } else { "More" })
                        .expanded(open)
                        .ml(3.5)
                        .cursor(Cursor::Pointer)
                        .focusable(true)
                        .focus_ring(2.0, 0.0, t.accent)
                        .child(text(if open { "Less" } else { "More" }).font(sans(rem(0.75), 400.0, 1.33)).color(t.accent_text))
                        .on_press(move |s: &mut State| s.settings.toggle_more(&toggled)),
                );
            }
            left = left.child(line);
        }
        if let (true, Some(words)) = (open, self.more) {
            left = left.child(
                text(words)
                    .key(format!("setting-more-text-{id}"))
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted)
                    .wrap()
                    .w_full()
                    .mt(3.5),
            );
        }
        if let Some(words) = self.adds {
            left = left.child(adds_line(t, &id, &words, self.open));
        }
        if let Some(below) = self.below {
            left = left.child(below);
        }
        let mut r =
            row().key(format!("settings-row-{id}")).w_full().items_start().justify_between().gap(14.0).px(14.0).py(10.5).child(left);
        if let Some(control) = self.control {
            r = r.child(el().child(control));
        }
        r
    }
}

/// "↳ Adds: <text> · Open", the Open underlined.
fn adds_line(t: &Theme, id: &str, words: &str, open: Option<Open>) -> El {
    let line = row().key(format!("setting-adds-line-{id}")).w_full().flex_wrap().mt(3.5).gap(4.0);
    let font = sans(rem(0.75), 400.0, 1.33);
    let line = line.child(
        text(format!("\u{21b3} Adds: {words}"))
            .key(format!("setting-adds-{id}"))
            .font(font)
            .color(t.accent_text)
            .wrap()
            .shrink()
            .min_w(0.0),
    );
    let Some(open) = open else { return line };
    line.child(text("\u{b7}").font(font).color(t.accent_text)).child(
        row()
            .key(format!("setting-open-{id}"))
            .role("button")
            .label("Open")
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 0.0, t.accent)
            .child(rich("Open", vec![Span::new(0, 4).underline(t.accent_text)]).font(font).color(t.accent_text))
            .on_press(move |s: &mut State| open(s)),
    )
}

/// What a switch is waiting for: `saving` shows a spinner, `pending` a ring (Mods asking to install Bun), `disabled` fades it.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwitchState {
    pub saving: bool,
    pub disabled: bool,
    pub pending: bool,
}

/// `SettingSwitch`: a 38.5 x 21 track, a 17.5 thumb. `saving` puts a spinner left of it; `pending` rings it. The caller
/// takes `press`. A disabled switch is faded and doesn't press.
pub fn setting_switch(t: &Theme, key: &str, label: &str, on: bool, state: SwitchState, press: impl Fn(&mut State) + 'static) -> El {
    let SwitchState { saving, disabled, pending } = state;
    let (w, h, thumb) = (38.5, 21.0, 17.5);
    let ring = if pending { t.accent } else { Color::TRANSPARENT };
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
        .border(2.0, ring)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .child(el().size(thumb, thumb).radius(thumb / 2.0).fill(Color::WHITE).ml(if on { w - thumb - 4.0 } else { 0.0 }));
    if disabled {
        track = track.opacity(0.6).disabled(true).cursor(Cursor::Default);
    } else {
        track = track.on_press(press);
    }
    let mut r = row().items_center().gap(7.0);
    if saving {
        r = r.child(spinner(t.muted));
    }
    r.child(track)
}

/// `SettingSegments`: a few choices side by side, one on. Arrow keys pick the neighbour and wrap. `key` names the group;
/// the group is `<key>-group` and each choice `<key>-<value>`. `disabled` shows progress.
pub fn segments(t: &Theme, key: &str, label: &str, options: &[(&str, &str)], value: &str, disabled: bool, pick: Pick) -> El {
    let values: Vec<String> = options.iter().map(|(v, _)| (*v).to_string()).collect();
    let current = values.iter().position(|v| v == value);
    let mut group = row()
        .key(format!("{key}-group"))
        .role("radiogroup")
        .label(label.to_string())
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .clip_rounded(RADIUS_BTN)
        .focus_group();
    if !disabled {
        let (values, pick) = (values.clone(), pick.clone());
        group = group.on_key(move |s: &mut State, ev: &KeyEvent, _| {
            let step: isize = match ev.key {
                Key::Right | Key::Down => 1,
                Key::Left | Key::Up => -1,
                _ => return false,
            };
            let Some(at) = current else { return false };
            let next = (at as isize + step).rem_euclid(values.len() as isize) as usize;
            pick(s, &values[next]);
            // The choice moves with the key; Rune's roving focus follows it.
            false
        });
    }
    for (at, (v, words)) in options.iter().enumerate() {
        let on = Some(at) == current;
        let (value, pick) = ((*v).to_string(), pick.clone());
        let mut seg = row()
            .key(format!("{key}-{v}"))
            .role("radio")
            .label((*words).to_string())
            .checked(on)
            .focus_selected(on)
            .px(11.0)
            .py(4.0)
            .focusable(true)
            .focus_ring(2.0, -2.0, t.accent)
            .cursor(if disabled { Cursor::Default } else { Cursor::Pointer })
            .child(text((*words).to_string()).font(sans(12.5, if on { 600.0 } else { 400.0 }, 1.4)).color(if on {
                t.text
            } else {
                t.muted
            }));
        if on {
            seg = seg.fill(t.accent.alpha(0.18));
        }
        if at > 0 {
            seg = seg.border_sides([0.0, 0.0, 0.0, 1.0], t.border);
        }
        seg = if disabled || on { seg.disabled(disabled) } else { seg.on_press(move |s: &mut State| pick(s, &value)) };
        group = group.child(seg);
    }
    group
}

/// `ErrorNote compact`: a 14px alert icon and the message, `mt-2` inside a row.
pub fn error_note(t: &Theme, key: &str, message: &str) -> El {
    row()
        .key(key.to_string())
        .role("alert")
        .mt(7.0)
        .gap(7.0)
        .items_start()
        .child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
        .child(text(message.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.error_text).wrap().shrink().min_w(0.0))
}
