//! Settings → Harnesses → Providers → a provider's sign-in flow (`HarnessSignInFlow.vue`): the methods as pills, the key or browser
//! form with the method's own fields, a browser sign-in under way with its code and paste boxes, and the advice for the command and
//! environment methods. Sizes are the component's own scoped styles, in CSS pixels.
use crate::fleet::settings::harness_sign_in::Provider;
use crate::fleet::settings::harness_sign_in::flow::{Attempt, Field, Flow, Method, code_to_enter, methods_to_offer, visible_fields};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::{Open, select};
use crate::{State, Tx};
use rune::{Cursor, El, col, el, icon, row, text, text_input};
use serde_json::Value;
use std::rc::Rc;

/// The open list of a field with options.
pub const FIELD_SELECT: &str = "sign-in-field";

fn body_text(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0)
}

/// A sentence with `code` pieces in it: `(words, is_code)`; each word is its own element so the line wraps.
fn prose(t: &Theme, key: &str, parts: &[(&str, bool)]) -> El {
    let whole: String = parts.iter().map(|(w, _)| *w).collect();
    let mut line = row().key(key.to_string()).label(whole).w_full().flex_wrap().items_start();
    for (words, code) in parts {
        let font = if *code { mono(11.5, 400.0, 1.5) } else { sans(12.5, 400.0, 1.5) };
        let pieces: Vec<&str> = if *code { vec![*words] } else { words.split_inclusive(' ').collect() };
        for piece in pieces {
            line = line.child(text(piece.to_string()).font(font).color(t.text));
        }
    }
    line
}

/// `.sign-in-flow__btn`: 1px border, `--radius-btn`, 5px 10px, 12px/500, `--card-bg`; the primary one in the accent.
fn button(t: &Theme, key: &str, words: &str, lead: Option<El>, primary: bool, on: bool) -> El {
    let (edge, fill, ink) =
        if primary { (t.accent.alpha(0.5), t.accent.alpha(0.14), t.accent_text) } else { (t.border, t.card_bg, t.text) };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .center_y()
        .gap(6.0)
        .px(10.0)
        .py(5.0)
        .radius(RADIUS_BTN)
        .border(1.0, edge)
        .border_layout()
        .fill(fill)
        .hover(t.accent.alpha(0.08))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent);
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    let b = b.child(text(words.to_string()).font(sans(12.0, 500.0, 1.5)).color(ink));
    if on { b } else { b.opacity(0.6).disabled(true).cursor(Cursor::Default) }
}

/// `.sign-in-flow__input`: 1px border, 6px radius, 6px 8px, `--card-bg`; the border takes the accent while it has focus.
fn boxed(t: &Theme, edit: &rune::TextEdit, field: El) -> El {
    field
        .grow(1.0)
        .min_w(0.0)
        .px(8.0)
        .py(6.0)
        .radius(6.0)
        .border(1.0, if edit.focused { t.accent.alpha(0.6) } else { t.border })
        .border_layout()
        .fill(t.card_bg)
}

fn mark(t: &Theme, on: bool) -> El {
    let b = el().size(13.0, 13.0).border(1.0, if on { t.accent } else { t.muted }).border_layout().radius(2.0);
    if on { b.fill(t.accent).center().child(icon(&CHECK, 10.0).color(t.primary_fg)) } else { b }
}

fn tick(t: &Theme, key: &str, on: bool, label: &str) -> El {
    let mut r =
        row().key(key.to_string()).role("checkbox").label(label.to_string()).checked(on).center_y().gap(6.0).cursor(Cursor::Pointer);
    r = r.focusable(true).focus_ring(2.0, 2.0, t.accent).child(mark(t, on));
    if label.is_empty() { r } else { r.child(text(label.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.text)) }
}

fn pills(t: &Theme, provider: &Provider, methods: &[Method], shown: &str) -> El {
    let mut r = row().role("tablist").label(format!("Ways to sign in to {}", provider.name)).flex_wrap().gap(6.0);
    for m in methods {
        let on = m.key() == shown;
        let (edge, fill, ink) =
            if on { (t.accent.alpha(0.45), t.accent.alpha(0.1), t.accent_text) } else { (t.border, t.card_bg, t.muted) };
        let key = m.key();
        r = r.child(
            row()
                .key(format!("sign-in-method-{key}"))
                .role("tab")
                .label(m.label.clone())
                .checked(on)
                .px(10.0)
                .py(3.0)
                .radius(999.0)
                .border(1.0, edge)
                .border_layout()
                .fill(fill)
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 0.0, t.accent)
                .on_press(move |s: &mut State| s.flow_pick_method(&key))
                .child(text(m.label.clone()).font(sans(12.0, 400.0, 1.5)).color(ink)),
        );
    }
    r
}

fn option_label(f: &Field, value: &Value) -> String {
    let at = value.as_str().unwrap_or_default();
    match f.options.iter().find(|o| o.value == at) {
        Some(o) => match &o.description {
            Some(d) => format!("{} ({d})", o.label),
            None => o.label.clone(),
        },
        None => at.to_string(),
    }
}

fn field_control(s: &State, t: &Theme, flow: &Flow, f: &Field, index: Option<usize>) -> El {
    let key = format!("sign-in-field-{}", f.key);
    let answer = flow.answers.get(&f.key).cloned().unwrap_or(Value::Null);
    if f.kind == "string" && !f.options.is_empty() {
        let k = f.key.clone();
        return row()
            .key(key)
            .role("combobox")
            .label(option_label(f, &answer))
            .w_full()
            .center_y()
            .justify_between()
            .gap(6.0)
            .px(8.0)
            .py(6.0)
            .radius(6.0)
            .border(1.0, if flow.choosing.as_deref() == Some(f.key.as_str()) { t.accent.alpha(0.6) } else { t.border })
            .border_layout()
            .fill(t.card_bg)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .on_press(move |s: &mut State| {
                if let Some(flow) = s.settings.harnesses.sign_in.flow.as_mut() {
                    flow.choosing = Some(k.clone());
                }
                s.ui.open = Some(Open::Select(FIELD_SELECT));
            })
            .child(text(option_label(f, &answer)).font(sans(12.5, 400.0, 1.5)).color(t.text).truncate().shrink())
            .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted));
    }
    match f.kind.as_str() {
        "boolean" => {
            let (k, on) = (f.key.clone(), answer.as_bool() == Some(true));
            tick(t, &key, on, "").on_press(move |s: &mut State| s.flow_set_answer(&k, Value::Bool(!on)))
        }
        "multiselect" => {
            let mut r = row().flex_wrap().gap(10.0);
            let chosen: Vec<&str> = answer.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            for o in &f.options {
                let on = chosen.contains(&o.value.as_str());
                let (k, v) = (f.key.clone(), o.value.clone());
                r = r.child(
                    tick(t, &format!("{key}-{}", o.value), on, &o.label).on_press(move |s: &mut State| s.flow_toggle_choice(&k, &v, !on)),
                );
            }
            r
        }
        "external" => {
            let url = f.url.clone().unwrap_or_default();
            let go = url.clone();
            row()
                .key(key)
                .role("link")
                .label(url.clone())
                .cursor(Cursor::Pointer)
                .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                    let _ = tx.open_url(&go);
                })
                .child(text(url).font(sans(12.5, 500.0, 1.5)).color(t.accent_text).wrap().shrink().min_w(0.0))
        }
        _ => {
            let _ = s;
            let i = index.unwrap_or(0);
            let edit = &flow.edits[i].1;
            let input = text_input(edit, sans(12.5, 400.0, 1.5))
                .key(key)
                .label(f.name().to_string())
                .single_line()
                .placeholder(f.placeholder.clone().unwrap_or_default(), t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .on_edit(move |s: &mut State| s.flow_edit_mut(i))
                .on_submit(|s: &mut State, tx: &mut Tx| s.flow_submit(tx));
            boxed(t, edit, input)
        }
    }
}

fn field(s: &State, t: &Theme, flow: &Flow, f: &Field) -> El {
    let index = flow.edit_index(&f.key);
    let mut c = col().w_full().gap(4.0).child(
        row()
            .child(text(f.name().to_string()).font(sans(12.5, 500.0, 1.5)).color(t.text))
            .child(text(if f.required { " *" } else { "" }.to_string()).font(sans(12.5, 500.0, 1.5)).color(t.text)),
    );
    c = c.child(field_control(s, t, flow, f, index));
    if let Some(d) = &f.description {
        c = c.child(text(d.clone()).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap());
    }
    c
}

fn form(s: &State, t: &Theme, flow: &Flow, provider: &Provider, method: &Method) -> El {
    let answers = flow.current();
    let mut form = col().w_full().gap(10.0);
    for f in visible_fields(&method.fields, &answers) {
        form = form.child(field(s, t, flow, f));
    }
    if method.kind == "key" {
        let input = text_input(&flow.key, sans(12.5, 400.0, 1.5))
            .key("sign-in-key")
            .label(method.label.clone())
            .single_line()
            .placeholder(format!("Paste your {} key", provider.name), t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .on_edit(|s: &mut State| s.flow_key_mut())
            .on_submit(|s: &mut State, tx: &mut Tx| s.flow_submit(tx));
        form = form.child(
            col()
                .w_full()
                .gap(4.0)
                .child(text(method.label.clone()).font(sans(12.5, 500.0, 1.5)).color(t.text))
                .child(row().w_full().child(boxed(t, &flow.key, input))),
        );
    } else {
        form = form.child(
            text(format!("{}\u{2019}s sign-in page opens in a new tab.", provider.name).replace('\u{2019}', "'"))
                .font(sans(12.5, 400.0, 1.5))
                .color(t.muted)
                .wrap(),
        );
    }
    let lead = flow.busy.then(|| spinner(t.accent_text));
    let words = if method.kind == "key" { "Sign in" } else { "Continue in the browser" };
    form.child(
        row()
            .flex_wrap()
            .gap(8.0)
            .child(button(t, "sign-in-submit", words, lead, true, !flow.busy).on_press_tx(|s: &mut State, tx: &mut Tx| s.flow_submit(tx)))
            .child(button(t, "sign-in-close", "Cancel", None, false, true).on_press(|s: &mut State| s.close_sign_in_flow())),
    )
}

fn attempt_view(_s: &State, t: &Theme, flow: &Flow, a: &Attempt) -> El {
    let name = &flow.provider_name;
    let go = a.url.clone();
    let mut c = col().key("sign-in-attempt").w_full().gap(10.0);
    c = c.child(
        row()
            .center_y()
            .gap(6.0)
            .child(spinner(t.muted))
            .child(body_text(t, &format!("Waiting for you to finish on {name}'s page. It runs out at {}.", a.expires))),
    );
    c = c.child(
        row()
            .key("sign-in-attempt-url")
            .role("link")
            .label(format!("Open {name}'s sign-in page"))
            .center_y()
            .gap(4.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                let _ = tx.open_url(&go);
            })
            .child(text(format!("Open {name}'s sign-in page")).font(sans(12.5, 500.0, 1.5)).color(t.accent_text))
            .child(icon(&EXTERNAL_LINK, 12.0).color(t.accent_text)),
    );
    if !a.instructions.is_empty() {
        let mut r = row()
            .key("sign-in-attempt-instructions")
            .w_full()
            .flex_wrap()
            .center_y()
            .gap(8.0)
            .px(8.0)
            .py(6.0)
            .radius(6.0)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.card_bg)
            .child(text(a.instructions.clone()).font(mono(12.0, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0));
        if code_to_enter(&a.instructions).is_some() {
            let copied = flow.copied_until.is_some();
            let glyph = icon(if copied { &CHECK } else { &COPY }, 12.0).color(t.text);
            r = r.child(
                button(t, "sign-in-copy-code", if copied { "Copied" } else { "Copy code" }, Some(glyph), false, true)
                    .on_press_tx(|s: &mut State, tx: &mut Tx| s.flow_copy_code(tx)),
            );
        }
        c = c.child(r);
    }
    if a.needs_code {
        let input = text_input(&flow.code, sans(12.5, 400.0, 1.5))
            .key("sign-in-code")
            .label("Code from the provider".to_string())
            .single_line()
            .placeholder("Paste the code from the provider".to_string(), t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .on_edit(|s: &mut State| s.flow_code_mut())
            .on_submit(|s: &mut State, _: &mut Tx| s.flow_submit_code());
        c =
            c.child(
                row().w_full().flex_wrap().gap(8.0).child(boxed(t, &flow.code, input)).child(
                    button(t, "sign-in-code-finish", "Finish", None, true, !flow.busy).on_press(|s: &mut State| s.flow_submit_code()),
                ),
            );
    }
    if let Some(address) = &a.callback {
        let remote = flow.remote;
        if remote {
            c = c.child(advice(
                t,
                "sign-in-remote-note",
                &[
                    (&format!("You're using Fleet from another device. When you approve, {name} sends your browser to "), false),
                    (address.as_str(), true),
                    (", which only opens on the machine Fleet runs on, so the page won't load here. Copy that page's address from the address bar and paste it below.", false),
                ],
            ));
        } else if !flow.paste_open {
            c = c.child(
                row()
                    .key("sign-in-paste-open")
                    .role("button")
                    .label("Page didn't load after you approved?")
                    .cursor(Cursor::Pointer)
                    .focusable(true)
                    .on_press(|s: &mut State| {
                        if let Some(f) = s.settings.harnesses.sign_in.flow.as_mut() {
                            f.paste_open = true;
                        }
                    })
                    .child(text("Page didn't load after you approved?").font(sans(12.0, 400.0, 1.5)).color(t.accent_text)),
            );
        }
        if flow.paste_open || remote {
            let input = text_input(&flow.landed, sans(12.5, 400.0, 1.5))
                .key("sign-in-landed-on")
                .label("Address of the page that didn't load".to_string())
                .single_line()
                .placeholder(format!("{address}?code=\u{2026}"), t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .on_edit(|s: &mut State| s.flow_landed_mut())
                .on_submit(|s: &mut State, _: &mut Tx| s.flow_submit_landed());
            let on = !flow.busy && !flow.landed.text.trim().is_empty();
            c = c.child(row().w_full().flex_wrap().gap(8.0).child(boxed(t, &flow.landed, input)).child(
                button(t, "sign-in-landed-finish", "Finish sign-in", None, true, on).on_press(|s: &mut State| s.flow_submit_landed()),
            ));
        }
    }
    c.child(row().child(button(t, "sign-in-cancel", "Cancel", None, false, true).on_press(|s: &mut State| s.flow_cancel())))
}

/// `.sign-in-flow__remote` and `__advice`: an accent-edged note, 6px radius, 8px 10px.
fn advice(t: &Theme, key: &str, parts: &[(&str, bool)]) -> El {
    col()
        .w_full()
        .px(10.0)
        .py(8.0)
        .radius(6.0)
        .border(1.0, t.accent.alpha(0.25))
        .border_layout()
        .fill(t.accent.alpha(0.06))
        .child(prose(t, key, parts))
}

fn command_advice(s: &State, t: &Theme, flow: &Flow, provider: &Provider, method: &Method) -> El {
    let _ = s;
    let run = method.command.join(" ");
    let mut c = col()
        .key("sign-in-command")
        .w_full()
        .gap(8.0)
        .px(10.0)
        .py(8.0)
        .radius(6.0)
        .border(1.0, t.accent.alpha(0.25))
        .border_layout()
        .fill(t.accent.alpha(0.06));
    c = c.child(prose(
        t,
        "sign-in-command-words",
        &[
            (&format!("{} signs in to {} by running ", flow.harness_name, provider.name), false),
            (run.as_str(), true),
            (". Fleet doesn't run commands for you: to use this way, sign in from a terminal on the machine Fleet runs on.", false),
        ],
    ));
    if let Some(command) = s.sign_in_command(&flow.harness) {
        c = c.child(
            row()
                .key("sign-in-command-box")
                .w_full()
                .px(8.0)
                .py(5.0)
                .radius(6.0)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.card_bg)
                .child(text(command).font(mono(11.5, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0)),
        );
    }
    c
}

fn env_advice(t: &Theme, flow: &Flow, provider: &Provider, method: &Method) -> El {
    let mut parts: Vec<(String, bool)> = vec![(format!("{} also reads {}'s key from ", flow.harness_name, provider.name), false)];
    let last = method.variables.len().saturating_sub(1);
    for (i, v) in method.variables.iter().enumerate() {
        if i > 0 {
            parts.push((if i == last { " or " } else { ", " }.to_string(), false));
        }
        parts.push((v.clone(), true));
    }
    parts.push((". Set it where Fleet starts, then restart Fleet.".to_string(), false));
    let refs: Vec<(&str, bool)> = parts.iter().map(|(w, c)| (w.as_str(), *c)).collect();
    advice(t, "sign-in-env", &refs)
}

/// The flow under a provider's article: border-top, 10px above, 10px between.
pub fn flow(s: &State, t: &Theme, kind: &str, provider: &Provider) -> Option<El> {
    let f = s.sign_in_flow(kind).filter(|f| f.provider == provider.id)?;
    let methods = methods_to_offer(provider);
    let method = methods.iter().find(|m| m.key() == f.method);
    let mut c = col().key(format!("sign-in-flow-{}", provider.id)).w_full().gap(10.0).pt(10.0);
    if methods.len() > 1 && f.attempt.is_none() {
        c = c.child(pills(t, provider, &methods, &f.method));
    }
    if let Some(a) = &f.attempt {
        c = c.child(attempt_view(s, t, f, a));
    } else if let Some(m) = method {
        c = c.child(match m.kind.as_str() {
            "key" | "oauth" => form(s, t, f, provider, m),
            "command" => command_advice(s, t, f, provider, m),
            "env" => env_advice(t, f, provider, m),
            _ => el(),
        });
    }
    if let Some(why) = &f.error {
        c = c.child(text(why.clone()).key("sign-in-error").role("alert").font(sans(12.5, 400.0, 1.5)).color(t.error).wrap());
    }
    Some(col().w_full().child(el().w_full().h(1.0).fill(t.border)).child(c))
}

/// The open list of the field being chosen from.
pub fn field_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(flow) = s.settings.harnesses.sign_in.flow.as_ref() else { return vec![] };
    let Some(method) = s.flow_method() else { return vec![] };
    let Some(f) = method.fields.iter().find(|f| Some(f.key.as_str()) == flow.choosing.as_deref()) else { return vec![] };
    let labels: Vec<(String, String)> = f
        .options
        .iter()
        .map(|o| (o.value.clone(), o.description.as_ref().map_or_else(|| o.label.clone(), |d| format!("{} ({d})", o.label))))
        .collect();
    let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let current = flow.answers.get(&f.key).and_then(Value::as_str).map(str::to_string);
    let key = f.key.clone();
    let choose: select::Choose = Rc::new(move |s: &mut State, value: &str| {
        s.ui.open = None;
        s.flow_set_answer(&key, Value::String(value.to_string()));
    });
    select::content(t, "sign-in-field-list", &format!("sign-in-field-{}", f.key), 280.0, &options, current.as_deref(), choose)
}
