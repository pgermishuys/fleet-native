//! Settings → Folders, the Worktrees card (`WorktreeNamingSection.vue`): the five fields that name a worktree, the token chips
//! under three of them, the "What you'd get" preview and Save. The names come from the same resolver the composer's plan line
//! uses; the preview is local and never asks the server.
use super::folders_locations::{pill, secondary};
use crate::State;
use crate::fleet::settings::naming_draft::{Field, NamingPage};
use crate::icons::CHECK;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::{confirm, rem};
use rune::{Align, El, Span, col, icon, rich, row, text, text_input};

const TEXT_SM: f32 = 12.25;

fn muted(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

/// `inputClass`: `rounded-btn border bg-main-bg px-3 py-2 font-mono text-[12.5px]`, the border goes to the accent on focus.
fn input(page: &NamingPage, t: &Theme, field: Field, placeholder: &str) -> El {
    let edit = page.edit(field);
    let mut b = text_input(edit, mono(12.5, 400.0, 1.43))
        .key(format!("naming-{}", field.key()))
        .label(field.label().to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_BTN)
        .border(1.0, if edit.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .disabled(page.loading)
        .on_edit(move |s: &mut State| s.settings.naming.edit_mut(field));
    if field == Field::Prefix {
        b = b.max_w(220.0);
    }
    b
}

/// `text-xs font-medium uppercase tracking-wide text-muted`.
fn label(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33).tracking(0.025 * rem(0.75))).color(t.muted)
}

/// A token chip: `{name}` then the hint; a click appends `{name}` to its field.
fn chip(t: &Theme, field: Field, name: &'static str, hint: &str) -> El {
    let shown = format!("{{{name}}} {hint}");
    row()
        .key(format!("naming-token-{}-{name}", field.key()))
        .role("button")
        .label(shown.clone())
        .px(7.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .child(text(shown).font(mono(11.0, 400.0, 1.5)).color(t.muted))
        .on_press(move |s: &mut State| s.insert_naming_token(field, name))
}

fn help(t: &Theme, words: &str) -> El {
    muted(t, words)
}

/// One labelled field: the label, the box, its chips or its help.
fn labelled(page: &NamingPage, t: &Theme, field: Field, placeholder: &str, below: Option<El>) -> El {
    let mut c = col().w_full().gap(5.25).child(label(t, field.label())).child(input(page, t, field, placeholder));
    let tokens = field.tokens();
    if !tokens.is_empty() {
        let mut chips = row().flex_wrap().gap(3.5);
        for token in tokens {
            chips = chips.child(chip(t, field, token.name, token.hint));
        }
        c = c.child(chips);
    }
    if let Some(b) = below {
        c = c.child(b);
    }
    c
}

fn ticket_help(t: &Theme) -> El {
    let lead = "Read from your message so ";
    let code = "{ticket}";
    let body = format!("{lead}{code} has a value. What it captures leaves the slug, so the key isn't said twice.");
    let spans = vec![Span::new(lead.len(), lead.len() + code.len()).mono().size(rem(0.75))];
    rich(body, spans).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)
}

fn intro(t: &Theme) -> El {
    let (a, code) =
        ("How new worktrees are named. Tokens are filled in when a session starts. A repository that commits its own ", "weave.jsonc");
    let body = format!("{a}{code} names branches its way instead, for everyone who clones it.");
    let spans = vec![Span::new(a.len(), a.len() + code.len()).mono().size(rem(0.75))];
    col()
        .w_full()
        .gap(3.5)
        .child(text("Worktrees").font(sans(TEXT_SM, 600.0, 1.43)).color(t.text))
        .child(rich(body, spans).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0))
}

/// A `dt`/`dd` pair of the preview: a 70px label and a mono value.
fn line(t: &Theme, key: &str, name: &str, value: &str, breaks: bool) -> El {
    let mut v = text(value.to_string()).key(key.to_string()).font(mono(13.0, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0);
    if breaks {
        v = v.wrap_anywhere();
    }
    row().w_full().gap(10.5).items_start().child(label(t, name).w(70.0).mt(3.0)).child(v)
}

/// `What you'd get`: the sample, then the name it makes.
fn preview(s: &State, page: &NamingPage, t: &Theme) -> El {
    let ms = crate::view::message::now_ms(s);
    let (y, m, d, ..) = crate::conv::time::civil(ms, 0);
    let got = page.preview(&format!("{y:04}-{:02}-{d:02}", m + 1));
    let sample = text_input(&page.sample, sans(TEXT_SM, 400.0, 1.43))
        .key("naming-sample")
        .label("A message to name from".to_string())
        .single_line()
        .placeholder("Describe a task…".to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .mt(7.0)
        .px(10.5)
        .py(7.0)
        .radius(RADIUS_BTN)
        .border(1.0, if page.sample.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.card_bg)
        .on_edit(|s: &mut State| &mut s.settings.naming.sample);
    let mut c = col()
        .key("naming-preview")
        .w_full()
        .mt(21.0)
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(text("WHAT YOU'D GET").font(sans(rem(0.75), 600.0, 1.33).tracking(0.025 * rem(0.75))).color(t.muted))
        .child(sample)
        .child(col().w_full().mt(10.5).gap(5.25).child(line(t, "naming-branch-line", "Branch", &got.branch_line(), false)).child(line(
            t,
            "naming-worktree-line",
            "Worktree",
            &got.worktree_line(),
            true,
        )));
    if got.session_id_stands_in {
        c = c.child(
            muted(
                t,
                "This message gives no slug, so the session id stands in for it — the name a session started without a message gets too.",
            )
            .key("naming-no-slug")
            .mt(7.0),
        );
    }
    c
}

fn buttons(page: &NamingPage, t: &Theme) -> El {
    let busy = page.saving;
    let lead = if busy { confirm::spinner(t.primary_fg) } else { row() };
    let save = pill(
        row().fill(t.accent),
        "naming-save",
        "Save",
        busy,
        lead,
        text("Save".to_string()).font(sans(TEXT_SM, 500.0, 1.43)).color(t.primary_fg),
        |s: &mut State| s.save_naming(),
    );
    let reset = pill(
        secondary(t),
        "naming-reset",
        "Use Fleet's defaults",
        busy,
        row(),
        text("Use Fleet's defaults".to_string()).font(sans(TEXT_SM, 500.0, 1.43)).color(t.text),
        |s: &mut State| s.reset_naming(),
    );
    let mut r = row().w_full().mt(17.5).gap(10.5).center_y().child(save).child(reset);
    if page.saved_until.is_some() {
        r = r.child(
            row()
                .key("naming-saved")
                .gap(3.5)
                .center_y()
                .child(icon(&CHECK, 15.0).color(t.running_text))
                .child(text("Saved".to_string()).font(sans(TEXT_SM, 400.0, 1.43)).color(t.running_text)),
        );
    }
    r
}

/// The card (`WorktreeNamingSection`).
pub fn card_view(s: &State, t: &Theme) -> El {
    let page = &s.settings.naming;
    let d = &page.defaults;
    let prefix_help = help(
        t,
        "What every branch starts with — your initials, your team, whatever your organization uses. The templates below are for anything more than that.",
    );
    let fields = col()
        .w_full()
        .mt(17.5)
        .gap(14.0)
        .child(labelled(page, t, Field::Prefix, &d.prefix, Some(prefix_help)))
        .child(labelled(page, t, Field::Branch, &d.branch, None))
        .child(labelled(page, t, Field::Root, &d.root, None))
        .child(labelled(page, t, Field::Folder, &d.folder, None))
        .child(labelled(page, t, Field::Ticket, "[A-Z]{2,}-\\d+", Some(ticket_help(t))));
    let mut c = col()
        .key("naming-card")
        .w_full()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .align(Align::Start)
        .child(intro(t))
        .child(fields)
        .child(preview(s, page, t));
    if let Some(why) = &page.error {
        c = c.child(col().w_full().mt(14.0).child(confirm::error_note(t, "naming-error", why, None)));
    }
    c.child(buttons(page, t))
}
