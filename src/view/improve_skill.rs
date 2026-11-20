//! Improve a skill: the dialog. Ports `components/skills/ImproveSkillDialog.vue` (max 588 wide): the note and the parts of
//! the session the model reads, the wait, the proposal as a diff (or edited by hand), Keep it. The state is
//! `fleet::improve`; the diff is `view::skill_diff`. Fleet edits by hand in CodeMirror; here it is the kit's text field in
//! mono (docs/RUNE.md). Sizes are the component's CSS pixels at Fleet's 14px root.
use super::report::field;
use super::skill_diff::skill_diff;
use crate::State;
use crate::composer::format::{format_count, format_tokens};
use crate::fleet::improve::{ImproveDialog, Stage};
use crate::fleet::skills::version_label;
use crate::icons::CHECK;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::{error_note, spinner};
use crate::ui::dialog::{close_x, lift, overlay};
use crate::ui::overlay::centred;
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, Key, KeyEvent, ScrollSpec, Spring, Transition, Tx, Vec2, col, el, icon, row, text};

const KEY: &str = "improve-skill";
/// `sm:max-w-2xl`, and the window less 16 px a side.
const WIDTH: f32 = 588.0;
/// `max-h-[88dvh]`.
const HEIGHT_OF_WINDOW: f32 = 0.88;

fn window(s: &State) -> (f32, f32) {
    let (w, h) = s.ui.sel.window;
    if w > 0.0 && h > 0.0 { (w, h) } else { (1280.0, 760.0) }
}

/// "~2.4k tokens", "~380 tokens": an estimate, so it says so (`aboutTokens`).
fn about(tokens: u64) -> String {
    format!("~{} tokens", format_tokens(Some(tokens as f64)))
}

/// The window and its scrim, to mount over the app; `None` while the dialog is closed.
pub fn dialog(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d = &s.skills.dialog;
    if !d.open {
        return None;
    }
    let (w, h) = window(s);
    let pad = rem(1.5);
    let max_h = h * HEIGHT_OF_WINDOW;
    let body = if d.stage == Stage::Proposal { proposal(s, t, d) } else { form(t, d) };
    let panel = col()
        .key("improve-skill-dialog")
        .role("dialog")
        .label(format!("Improve {}", d.skill))
        .w(WIDTH.min(w - 32.0))
        .max_h(max_h)
        .padding(pad)
        .gap(rem(1.0))
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .scroll(ScrollSpec { stick_bottom: false })
        .child(head(t, d))
        .child(body)
        .child(close_x(t, KEY, |s: &mut State| s.improve_close()));
    // Esc and the overlay close it, except while a version is being saved.
    let panel = lift(t, panel)
        .transition(Transition::scale_anchor(0.95, Vec2::new(0.5, 0.5)))
        .token_motion(Spring::new(0.2, 0.0))
        .modal(!d.saving, |s: &mut State| s.improve_close());
    Some(vec![overlay(KEY), centred("improve-skill-frame", panel)])
}

fn head(t: &Theme, d: &ImproveDialog) -> El {
    let mut title = row()
        .key("improve-skill-title")
        .flex_wrap()
        .gap(rem(0.5))
        .center_y()
        .child(text("Improve".to_string()).font(sans(rem(1.125), 600.0, 1.0)).color(t.text))
        .child(text(d.skill.clone()).font(mono(rem(1.125), 600.0, 1.0)).color(t.text));
    if let Some(detail) = &d.detail {
        let yours = detail.version.is_some();
        let pill = row()
            .key("improve-skill-pill")
            .px(7.0)
            .radius(999.0)
            .border(1.0, if yours { rune::Color::TRANSPARENT } else { t.border })
            .border_layout()
            .child(text(version_label(detail.version)).font(sans(11.0, 600.0, 1.5)).color(if yours { t.accent_text } else { t.muted }));
        title = title.child(if yours { pill.fill(t.accent_dim) } else { pill });
    }
    let words = if d.stage == Stage::Proposal {
        let next = d.detail.as_ref().map_or(1, |x| x.next_version());
        let replaces = match d.detail.as_ref().and_then(|x| x.version) {
            Some(v) => format!("your version {v}"),
            None => "Fleet's".to_string(),
        };
        format!("Keep it to make it your version {next}: sessions you start from now on use it in place of {replaces}.")
    } else {
        "Say what it should do differently. The model proposes a change to the skill, and nothing is saved until you keep it.".to_string()
    };
    col()
        .gap(rem(0.5))
        .child(title)
        .child(text(words).key("improve-skill-about").font(sans(rem(0.875), 400.0, 1.4)).color(t.muted).wrap().w_full())
}

// ---- the form -------------------------------------------------------------------------------------------------------

/// One line of "What the model reads": a box, what it is, and what it costs. `on` is `None` for a box that is always on.
fn reads(t: &Theme, key: &str, words: El, cost: &str, on: bool, press: Option<fn(&mut State)>, label: &str) -> El {
    let live = press.is_some();
    let mark = el().size(16.0, 16.0).radius(3.0).border(1.0, if on { t.accent } else { t.muted }).border_layout();
    let mark = if on { mark.fill(t.accent).center().child(icon(&CHECK, 12.0).color(t.primary_fg)) } else { mark };
    let line = row()
        .key(key.to_string())
        .role("checkbox")
        .label(label.to_string())
        .checked(on)
        .w_full()
        .gap(10.0)
        .px(8.0)
        .py(6.0)
        .center_y()
        .radius(6.0)
        .child(mark)
        .child(el().grow(1.0).min_w(0.0).child(words))
        .child(text(cost.to_string()).font(mono(11.5, 400.0, 1.4).features("'tnum' 1")).color(t.muted));
    match press {
        Some(f) => line.cursor(Cursor::Pointer).hover(t.wash(4.0)).focus_ring(2.0, 2.0, t.accent).on_press(f),
        None if !live => line.disabled(true),
        None => line,
    }
}

fn plain(words: &str, ink: rune::Color) -> El {
    text(words.to_string()).font(sans(13.0, 400.0, 1.4)).color(ink)
}

fn form(t: &Theme, d: &ImproveDialog) -> El {
    let asking = d.stage == Stage::Asking;
    let note = field(t, "improve-skill-note", &d.note, "e.g. Don't report naming or style as findings. fleet-simplify covers those.", 3)
        .on_edit(|s: &mut State| &mut s.skills.dialog.note)
        // The field takes the key before the frame does, so it is asked through the field.
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            if ev.key == Key::Enter && (ev.mods.cmd || ev.mods.ctrl) {
                s.improve_ask();
                return true;
            }
            false
        });
    let note = if asking { disabled(note) } else { note };
    let note_field = col()
        .gap(rem(0.5))
        .child(text("What should it do differently?".to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text))
        .child(note);

    let off_exchange = d.whole || d.turn.exchange.is_empty();
    let off_tools = d.whole || d.turn.tool_calls.is_empty();
    let ink = |off: bool| if off { t.muted } else { t.text };
    let code = |w: &str| text(w.to_string()).font(mono(12.0, 400.0, 1.4)).color(t.text);
    let mut list = col()
        .key("improve-skill-reads")
        .w_full()
        .gap(2.0)
        .child(text("What the model reads".to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text).mb(6.0))
        .child(reads(
            t,
            "improve-skill-include-skill",
            row().child(plain("The skill, ", t.text)).child(code("SKILL.md")),
            &about(d.skill_tokens()),
            true,
            None,
            "The skill, SKILL.md",
        ))
        .child(reads(t, "improve-skill-include-note", plain("Your note", t.text), &about(d.note_tokens()), true, None, "Your note"))
        .child(reads(
            t,
            "improve-skill-include-exchange",
            plain("What you asked, and the reply", ink(off_exchange)),
            &about(d.exchange_tokens()),
            d.exchange && !off_exchange,
            (!off_exchange && !asking).then_some((|s: &mut State| s.skills.dialog.exchange ^= true) as fn(&mut State)),
            "What you asked, and the reply",
        ))
        .child(reads(
            t,
            "improve-skill-include-tools",
            plain("The tool calls in that turn", ink(off_tools)),
            &about(d.tool_tokens()),
            d.tools && !off_tools,
            (!off_tools && !asking).then_some((|s: &mut State| s.skills.dialog.tools ^= true) as fn(&mut State)),
            "The tool calls in that turn",
        ))
        .child(reads(
            t,
            "improve-skill-whole-conversation",
            plain("The whole conversation, from a copy of this session", t.text),
            "whole session",
            d.whole,
            (!asking).then_some((|s: &mut State| s.skills.dialog.whole ^= true) as fn(&mut State)),
            "The whole conversation, from a copy of this session",
        ));
    let total = if d.whole { format!("{} + the session", about(d.total_tokens())) } else { about(d.total_tokens()) };
    list = list.child(
        row()
            .w_full()
            .justify_between()
            .center_y()
            .gap(12.0)
            .mt(4.0)
            .px(8.0)
            .pt(8.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
            .child(text("Asked once, on this session's model".to_string()).font(sans(12.5, 400.0, 1.4)).color(t.muted))
            .child(text(total).key("improve-skill-total").font(sans(12.5, 400.0, 1.4)).color(t.muted)),
    );
    let mut form = col().w_full().gap(rem(1.0)).child(note_field).child(list);
    if let Some(why) = &d.error {
        form = form.child(error_note(t, "improve-skill-error", why, None));
    }
    let ask = if asking {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg)), "Asking the model\u{2026}")
    } else {
        button(t, Variant::Default, Size::Default, None, "Propose a change")
    }
    .key("improve-skill-ask");
    let ask = if asking || !d.can_ask() { disabled(ask) } else { ask.on_press(|s: &mut State| s.improve_ask()) };
    form.child(
        row()
            .w_full()
            .justify_end()
            .gap(rem(0.5))
            .child(
                button(t, Variant::Outline, Size::Default, None, "Cancel")
                    .key("improve-skill-cancel")
                    .on_press(|s: &mut State| s.improve_close()),
            )
            .child(ask),
    )
}

// ---- the proposal ---------------------------------------------------------------------------------------------------

fn proposal(s: &State, t: &Theme, d: &ImproveDialog) -> El {
    let Some(p) = &d.proposal else { return col() };
    let why = row()
        .key("improve-skill-why")
        .pl(10.0)
        .border_sides([0.0, 0.0, 0.0, 2.0], t.accent)
        .child(text(format!("\u{201c}{}\u{201d}", d.note.text.trim())).font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    let shown = if d.editing {
        // The field grows with its text and the box around it scrolls, as the diff's does: the kit's field doesn't clip
        // or scroll its own text.
        let rows: usize = d.draft.text.split('\n').map(|l| l.chars().count().div_ceil(76).max(1)).sum::<usize>() + 1;
        el().key("improve-skill-editor-box")
            .w_full()
            .max_h(340.0)
            .scroll(ScrollSpec { stick_bottom: false })
            .fill(t.card_bg)
            .radius(RADIUS_BTN)
            .border(1.0, t.border)
            .border_layout()
            .child(
                text_input(&d.draft, mono(12.0, 400.0, 1.5))
                    .key("improve-skill-editor")
                    .label(format!("{} SKILL.md", d.skill))
                    .color(t.text)
                    .caret(t.text, t.accent.alpha(0.35))
                    .w_full()
                    .px(10.0)
                    .py(8.0)
                    .min_h(rows as f32 * 18.0 + 16.0)
                    .on_edit(|s: &mut State| &mut s.skills.dialog.draft),
            )
    } else {
        skill_diff(t, s, "improve-skill-diff", &p.base, &d.draft.text, &format!("{}/SKILL.md", d.skill))
    };
    let mut body = col().w_full().gap(rem(1.0)).child(why).child(shown);
    if let Some(tokens) = p.tokens {
        let mut line = format!("The question used {} tokens", format_count(Some(tokens.total as f64)));
        if tokens.from_cache > 0 {
            line.push_str(&format!(" ({} from the cache)", format_count(Some(tokens.from_cache as f64))));
        }
        line.push('.');
        body = body.child(text(line).key("improve-skill-used").font(sans(rem(0.75), 400.0, 1.4)).color(t.muted));
    }
    if let Some(why) = &d.error {
        body = body.child(error_note(t, "improve-skill-error", why, None));
    }
    let off = |e: El, off: bool| if off { disabled(e) } else { e };
    let keep = if d.saving {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg)), "Keep it")
    } else {
        button(t, Variant::Default, Size::Default, None, "Keep it")
    }
    .key("improve-skill-keep");
    let keep = if d.can_keep() { keep.on_press(|s: &mut State| s.improve_keep()) } else { disabled(keep) };
    body.child(
        row()
            .w_full()
            .flex_wrap()
            .justify_end()
            .gap(rem(0.5))
            .child(off(
                button(t, Variant::Outline, Size::Default, None, "Change the note")
                    .key("improve-skill-change-note")
                    .on_press(|s: &mut State| s.skills.dialog.stage = Stage::Compose),
                d.saving,
            ))
            .child(off(
                button(t, Variant::Outline, Size::Default, None, if d.editing { "Show the diff" } else { "Edit by hand" })
                    .key("improve-skill-edit")
                    .on_press(|s: &mut State| s.skills.dialog.editing ^= true),
                d.saving,
            ))
            .child(keep),
    )
}
