//! `NSD/new-session.css`: the picker chip under the New session box and the `ns-pop` popover every picker opens (rows
//! B11.1-B11.14). One look for the machine, folder, workspace, base, harness, profile and "…" pickers and the starters menu.
use crate::State;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Icon, col, el, icon, rich, row, text};

/// What a chip shows, in order: a leading mark, the label, a hint, a chevron (B11.6).
pub struct Chip {
    key: String,
    label: String,
    glyph: Option<&'static Icon>,
    /// A 7px status dot in place of the glyph (the machine chip); the colour is the caller's.
    dot: Option<rune::Color>,
    /// The label in its own ink instead of `--text` (the machine chip when it can't be reached).
    ink: Option<rune::Color>,
    hint: Option<String>,
    attention: bool,
    open: bool,
    disabled: bool,
    help: Option<String>,
    label_font: Option<rune::tok::TextStyle>,
    /// A spinner in place of the glyph (a clone for this folder runs).
    busy: bool,
    /// `ns-chip__new`: the small `new` tag after the label.
    tag: bool,
    /// `ns-chip__hint` inside the label: muted words before (`from `) and after (` · no fetch`) it, weight 400.
    head: Option<String>,
    tail: Option<String>,
    /// The last mark instead of the chevron (the "…" chip's ellipsis).
    trailing: Option<&'static Icon>,
    /// No chevron: a chip that flips something rather than opening a menu.
    flat: bool,
    /// Nothing to press: a static chip (New worktree) has no hover wash and no pointer.
    still: bool,
}

impl Chip {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Chip {
        Chip {
            key: key.into(),
            label: label.into(),
            glyph: None,
            dot: None,
            ink: None,
            hint: None,
            attention: false,
            open: false,
            disabled: false,
            help: None,
            label_font: None,
            busy: false,
            tag: false,
            head: None,
            tail: None,
            trailing: None,
            flat: false,
            still: false,
        }
    }
    pub fn flat(mut self) -> Self {
        self.flat = true;
        self
    }
    pub fn still(mut self) -> Self {
        self.still = true;
        self
    }
    pub fn glyph(mut self, g: &'static Icon) -> Self {
        self.glyph = Some(g);
        self
    }
    pub fn dot(mut self, color: rune::Color) -> Self {
        self.dot = Some(color);
        self
    }
    pub fn ink(mut self, color: rune::Color) -> Self {
        self.ink = Some(color);
        self
    }
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    /// `ns-chip--attention`: nothing is chosen yet.
    pub fn attention(mut self, on: bool) -> Self {
        self.attention = on;
        self
    }
    /// The menu from this chip is open (`aria-expanded`).
    pub fn open(mut self, on: bool) -> Self {
        self.open = on;
        self
    }
    pub fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
    /// The whole value as the chip's tooltip (the folder chip's `title`).
    pub fn help(mut self, text: Option<String>) -> Self {
        self.help = text;
        self
    }
    pub fn busy(mut self, on: bool) -> Self {
        self.busy = on;
        self
    }
    pub fn new_tag(mut self, on: bool) -> Self {
        self.tag = on;
        self
    }
    pub fn head(mut self, words: impl Into<String>) -> Self {
        self.head = Some(words.into());
        self
    }
    pub fn tail(mut self, words: Option<String>) -> Self {
        self.tail = words;
        self
    }
    pub fn trailing(mut self, g: &'static Icon) -> Self {
        self.trailing = Some(g);
        self
    }
    pub fn label_font(mut self, font: rune::tok::TextStyle) -> Self {
        self.label_font = Some(font);
        self
    }

    /// `.ns-chip`: 26 high, 5px gap, `0 7px`, 8px radius, 12.5px; hover and open wash `--text` 6%.
    pub fn view(self, t: &Theme, on_press: impl Fn(&mut State, &mut crate::Tx) + 'static) -> El {
        let washed = self.open && !self.disabled;
        let mut chip = row()
            .key(self.key)
            .role("button")
            .label(self.label.clone())
            .h(26.0)
            .px(7.0)
            .gap(5.0)
            .center_y()
            .radius(RADIUS_BTN)
            .border(1.0, if self.attention { t.border_mix(t.accent, 45.0) } else { rune::Color::TRANSPARENT })
            .shrink()
            .min_w(0.0)
            .hover_group()
            .focus_ring(2.0, 1.0, t.accent);
        if self.disabled {
            chip = chip.disabled(true).opacity(0.6);
        } else if self.still {
            // Static: no wash, no pointer, nothing to press.
        } else {
            chip = chip.hover(t.wash(6.0)).cursor(Cursor::Pointer).on_press_tx(on_press);
        }
        if washed {
            chip = chip.fill(t.wash(6.0));
        }
        if let Some(text) = self.help {
            chip = chip.help(text);
        }
        // The glyph and chevron are `--muted` and turn to `--text` with the chip's hover; the label is always `--text`.
        let mark = |g: &'static Icon, size: f32, opacity: f32| icon(g, size).color(t.muted).group_color(t.text).opacity(opacity);
        if self.busy {
            static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
            chip = chip.child(icon(&crate::icons::LOADER_CIRCLE, 14.0).color(t.muted).cycle(rune::Cycle {
                period: 1.2,
                phase: 0.0,
                stops: &FADE,
                still: 1.0,
                fps: 30.0,
            }));
        } else if let Some(color) = self.dot {
            chip = chip.child(el().size(7.0, 7.0).radius(999.0).fill(color));
        } else if let Some(g) = self.glyph {
            chip = chip.child(mark(g, 14.0, 1.0));
        }
        let ink = self.ink.unwrap_or(t.text);
        let font = self.label_font.unwrap_or(sans(12.5, 500.0, 1.2));
        if self.head.is_some() || self.tail.is_some() {
            let (head, tail) = (self.head.unwrap_or_default(), self.tail.unwrap_or_default());
            let whole = format!("{head}{}{tail}", self.label);
            let mut spans = Vec::new();
            if !head.is_empty() {
                spans.push(rune::Span::new(0, head.len()).weight(400.0).color(t.muted));
            }
            if !tail.is_empty() {
                spans.push(rune::Span::new(whole.len() - tail.len(), whole.len()).weight(400.0).color(t.muted));
            }
            chip = chip.child(rich(whole, spans).font(font).color(ink).truncate().shrink());
        } else if !self.label.is_empty() {
            chip = chip.child(text(self.label).font(font).color(ink).truncate().shrink());
        }
        if self.tag {
            // `.ns-chip__new`: 10px/600 on a 16px line, `0 4px`, 4px radius, `--running` at 14%.
            chip = chip.child(
                row()
                    .key("new-session-folder-new")
                    .px(4.0)
                    .h(16.0)
                    .center_y()
                    .radius(4.0)
                    .fill(t.running.alpha(0.14))
                    .child(text("new").font(sans(10.0, 600.0, 1.6)).color(t.running_text)),
            );
        }
        if let Some(hint) = self.hint {
            chip = chip.child(text(hint).font(sans(12.5, 400.0, 1.2)).color(t.muted));
        }
        match self.trailing {
            Some(g) => chip.child(mark(g, 14.0, 1.0)),
            None if self.flat => chip,
            None => chip.child(mark(&crate::icons::CHEVRON_DOWN, 12.0, 0.5)),
        }
    }
}

/// `.ns-pop`: the popover panel every picker opens, `width` wide (344 unless the picker says otherwise, B11.8), 5px padding,
/// 10px radius, `--card-bg`, the literal shadow, 13px.
pub fn pop(t: &Theme, key: &str, width: f32, children: Vec<El>) -> El {
    col()
        .key(key.to_string())
        .role("menu")
        .focus_group()
        .w(width)
        .padding(5.0)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .items_stretch()
        .box_shadow(t.pop_shadow())
        .children(children)
}

/// `.ns-pop__label`: `7px 8px 3px`, 11px/600, tracked .05em, upper case, `--muted`.
pub fn pop_label(t: &Theme, words: &str) -> El {
    row().px(8.0).pt(7.0).pb(3.0).child(text(words.to_uppercase()).font(sans(11.0, 600.0, 1.2).tracking(0.55)).color(t.muted))
}

/// `.ns-pop__separator`: 1px, `4px 2px`.
pub fn pop_separator(t: &Theme) -> El {
    el().h(1.0).mx(2.0).my(4.0).fill(t.border)
}

/// `.ns-pop__note`: `--muted`. `new-session.css` says 8px and 12.5px, but ProfilePicker's unscoped style redefines the rule
/// (`4px 8px 6px`, 11.5px) and Fleet's client always loads it, so every note is drawn the smaller way (B11.11, a Fleet issue).
pub fn pop_note(t: &Theme, words: &str) -> El {
    row().px(8.0).pt(4.0).pb(6.0).child(text(words).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().w_full())
}

/// The parts of `.ns-option` besides its click: a 16px mark column, two lines of text, a 14px check column.
pub struct Option_ {
    pub key: String,
    pub icon: Option<&'static Icon>,
    pub title: String,
    pub detail: Option<String>,
    /// The detail in JetBrains Mono 11px (`ns-option__detail--mono`).
    pub mono_detail: bool,
    pub checked: bool,
    pub disabled: bool,
    /// The folder menu's highlighted row (`[data-highlighted]`): the same wash as hover.
    pub highlighted: bool,
    /// The part of the title in bold (`Create` **`clients/portal`**).
    pub bold: Option<String>,
    /// A key hint in the last column instead of the check (`.ns-folder-kbd`: the path view's highlighted row).
    pub hint: Option<String>,
    /// `ns-option__tag`: muted words after the title (` · default`), 11.5px, weight 400.
    pub tag: Option<String>,
    /// No mark column at all (the harness and profile rows are `1fr 14px`).
    pub no_mark: bool,
    /// The title in JetBrains Mono 12px (the machine rows' names).
    pub mono_title: bool,
}

impl Option_ {
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Option_ {
        Option_ {
            key: key.into(),
            icon: None,
            title: title.into(),
            detail: None,
            mono_detail: false,
            checked: false,
            disabled: false,
            highlighted: false,
            bold: None,
            hint: None,
            tag: None,
            no_mark: false,
            mono_title: false,
        }
    }
}

/// `.ns-option`: grid `16 1fr 14`, 8px gap, `6px 8px`, 6px radius, 13px; highlight `--text` 6% on hover and on the focused
/// row. `lead` replaces the icon column (the machine dot).
pub fn option(t: &Theme, o: Option_, lead: Option<El>, on_press: impl Fn(&mut State, &mut crate::Tx) + 'static) -> El {
    let wash = t.wash(6.0);
    let mut r = row()
        .key(o.key)
        .role("menuitem")
        .label(o.title.clone())
        .w_full()
        .px(8.0)
        .py(6.0)
        .gap(8.0)
        .center_y()
        .radius(RADIUS_BTN - 2.0)
        .cursor(Cursor::Default)
        .focusable(!o.disabled);
    if o.disabled {
        // `.ns-option[data-disabled]`: `opacity .55`, no pointer.
        r = r.opacity(0.55);
    } else {
        r = r.hover(wash).focus_highlight(wash).on_press_tx(on_press);
    }
    if o.highlighted && !o.disabled {
        r = r.fill(wash);
    }
    let lead = lead.or_else(|| o.icon.map(|g| icon(g, 14.0).color(t.muted)));
    if !o.no_mark {
        r = r.child(el().w(16.0).center().children(lead));
    }
    let font = if o.mono_title { mono(12.0, 400.0, 1.3) } else { sans(13.0, 400.0, 1.3) };
    let title = match (o.bold, o.tag) {
        (_, Some(tag)) => {
            let spans = vec![rune::Span::new(o.title.len(), o.title.len() + tag.len()).weight(400.0).size(11.5).color(t.muted)];
            rich(format!("{}{tag}", o.title), spans).font(font).color(t.text).truncate()
        }
        (Some(bold), None) => {
            let spans = vec![rune::Span::new(o.title.len() + 1, o.title.len() + 1 + bold.len()).weight(600.0)];
            rich(format!("{} {bold}", o.title), spans).font(font).color(t.text).truncate()
        }
        (None, None) => text(o.title).font(font).color(t.text).truncate(),
    };
    let mut words = col().grow(1.0).min_w(0.0).child(title);
    if let Some(detail) = o.detail {
        let font = if o.mono_detail { mono(11.0, 400.0, 1.3) } else { sans(11.5, 400.0, 1.3) };
        words = words.child(text(detail).font(font).color(t.muted).truncate());
    }
    r = r.child(words);
    if let Some(hint) = o.hint {
        // `.ns-folder-kbd`: padding `0 4px`, 1px border, radius 4, 10.5px, line-height 16.
        return r.child(
            row()
                .px(4.0)
                .radius(4.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text(hint).font(sans(10.5, 400.0, 16.0 / 10.5)).color(t.muted)),
        );
    }
    r.child(el().w(14.0).center().children(o.checked.then(|| icon(&crate::icons::CHECK, 14.0).color(t.accent))))
}

/// `.ns-field`: a label over its box, `6px 8px 8px`, 4px apart.
pub fn pop_field(t: &Theme, label: &str, input: El) -> El {
    col().w_full().gap(4.0).px(8.0).pt(6.0).pb(8.0).child(text(label).font(sans(11.5, 400.0, 1.3)).color(t.muted)).child(input)
}

/// `.ns-field__input`: padding `6px 8px`, 1px border, 6px radius, the text at 3%; the border turns accent while the field
/// inside has the keys.
pub fn pop_field_box(t: &Theme, input: El) -> El {
    row()
        .w_full()
        .px(8.0)
        .py(6.0)
        .items_center()
        .radius(RADIUS_BTN - 2.0)
        .border(1.0, t.border)
        .fill(t.wash(3.0))
        .focus_within_ring(1.0, -1.0, t.accent)
        .child(input)
}
