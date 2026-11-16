//! The Allowed chip in the composer's toolbar and the list it opens. Ports `AllowedChip.vue`, `AllowedRulesPanel.vue` and
//! `AllowedRulesList.vue` (docs/parity/04-questions-and-permissions.md, rows 5.1-5.10). What a rule says is worked out
//! in `asks::wording`; the store is `fleet::rules`.
use crate::State;
use crate::asks::rules::{AllowedRule, PermissionLevel};
use crate::asks::wording::allowed_rule_wording;
use crate::icons::SHIELD_CHECK;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::dialog::lift;
use crate::ui::overlay::{anchored, scrim};
use crate::ui::{Open, age, rem};
use rune::{Align, Cursor, El, Side, Span, col, icon, rich, row, text};

/// `w-[22rem]`.
const WIDTH: f32 = 308.0;
/// How long the chip's background takes to fade after a rule is added (`allowed-flash`).
pub const FLASH_SECS: f64 = 0.9;

/// "Allowed · N" in the toolbar; none while the session has no rule.
pub fn chip(s: &State, t: &Theme) -> Option<El> {
    let count = s.open_rules().len();
    if count == 0 {
        return None;
    }
    let open = s.ui.open == Some(Open::Popover("allowed"));
    let mut b = row()
        .key("allowed-chip")
        .role("button")
        .label(format!("Allowed · {count}"))
        .h(28.0)
        .px(8.0)
        .gap(3.5)
        .center_y()
        .radius(crate::theme::RADIUS_BTN)
        .hover(t.wash(6.0))
        .hover_group()
        .focus_ring(2.0, 2.0, t.accent)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.open_allowed());
    // `allowed-flash`: the primary colour at 35%, fading out over 0.9 s.
    if let Some(alpha) = flash_alpha(s) {
        b = b.fill(t.accent.alpha(alpha));
    } else if open {
        b = b.fill(t.wash(6.0));
    }
    // The tooltip is off while the list is open.
    if !open {
        b = b.help("What you told the agent not to ask about again");
    }
    // The scoped style says `font-size: 0.75rem`, which is 10.5 px on Fleet's 14 px root (not the 12 px the checklist said).
    let ink = if open { t.text } else { t.muted };
    Some(
        b.child(icon(&SHIELD_CHECK, 12.25).color(ink).group_color(t.text))
            .child(text(format!("Allowed · {count}")).font(sans(rem(0.75), 500.0, 1.2)).color(ink).group_color(t.text)),
    )
}

/// The chip's flash at this moment: 35% falling to nothing, eased out (`ease-out`); none when it isn't flashing.
fn flash_alpha(s: &State) -> Option<f32> {
    let since = s.clock - s.allowed_flash?;
    (0.0..FLASH_SECS).contains(&since).then(|| {
        let left = 1.0 - since / FLASH_SECS;
        0.35 * (left * left) as f32
    })
}

/// The scrim and the card above the chip (`side="top" align="start" :side-offset="8"`, `w-[22rem]`, `p-4`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let card = lift(
        t,
        col()
            .key("allowed-popover")
            .role("dialog")
            .label("Allowed")
            .w(WIDTH)
            .padding(14.0)
            .fill(t.card_bg)
            .border(1.0, t.border)
            .border_layout()
            .child(list(s, t, s.open_rules(), s.open_level())),
    )
    .focus_return("allowed-chip".to_string())
    .modal(true, |s: &mut State| s.ui.open = None);
    vec![scrim("allowed-scrim", |s: &mut State| s.ui.open = None), anchored(card, "allowed-chip", Side::Top, Align::Start, 8.0)]
}

/// `AllowedRulesList`: one row per rule, a note when asking is off, and Remove all from two rules.
pub fn list(s: &State, t: &Theme, rules: &[AllowedRule], level: PermissionLevel) -> El {
    let dormant = level == PermissionLevel::All;
    let now = crate::view::message::now_ms(s);
    let offset = crate::conv::time::local_offset_secs();
    let mut out = col().key("allowed-list").w_full().gap(7.0);
    if dormant {
        out = out.child(
            text("Not needed now: this session allows everything. They count again if you turn asking back on.")
                .key("allowed-dormant")
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    }
    let mut rows = col().key("allowed-rows").w_full().opacity(if dormant { 0.55 } else { 1.0 });
    for (i, rule) in rules.iter().enumerate() {
        rows = rows.child(rule_row(t, rule, i > 0, now, offset));
    }
    out = out.child(rows);
    if rules.len() > 1 {
        out = out.child(
            row().w_full().justify_end().border_sides([1.0, 0.0, 0.0, 0.0], t.border).pt(3.5).child(
                button(t, Variant::Ghost, Size::Sm, None, "Remove all")
                    .key("allowed-remove-all")
                    .on_press(|s: &mut State| s.remove_all_rules()),
            ),
        );
    }
    out
}

fn rule_row(t: &Theme, rule: &AllowedRule, divided: bool, now: i64, offset: i64) -> El {
    let patterns: Vec<&str> = rule.patterns.iter().map(String::as_str).collect();
    let words = allowed_rule_wording(rule.kind, &rule.tool, &patterns);
    // The wording is one run of text that wraps anywhere; the code part is mono 12 px.
    let mut content = words.lead.clone();
    let mut spans = Vec::new();
    if let Some(code) = &words.code {
        content.push(' ');
        spans.push(Span::new(content.len(), content.len() + code.len()).mono().size(rem(0.75)));
        content.push_str(code);
    }
    content.push_str(words.tail);
    let what = rich(content, spans)
        .key(format!("allowed-rule-{}-what", rule.id))
        .font(sans(rem(0.8125), 400.0, 1.5))
        .color(if words.broad { t.warning() } else { t.text })
        .wrap()
        .w_full();
    let age_text = age::compact(&rule.created_at, now, offset);
    let mut text_col = col().grow(1.0).shrink().min_w(0.0).gap(1.4).child(what);
    if !age_text.is_empty() {
        text_col = text_col.child(
            text(age_text).key(format!("allowed-age-{}", rule.id)).font(sans(rem(0.6875), 400.0, 1.4)).color(t.muted).help(age::exact(
                &rule.created_at,
                now,
                offset,
            )),
        );
    }
    let id = rule.id.clone();
    let remove = button(t, Variant::Ghost, Size::Sm, None, "Remove")
        .key(format!("allowed-remove-{}", rule.id))
        .on_press(move |s: &mut State| s.remove_rule(&id));
    let mut r = row().key(format!("allowed-rule-{}", rule.id)).w_full().center_y().gap(10.5).py(5.6).child(text_col).child(remove);
    if divided {
        r = r.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    r
}
