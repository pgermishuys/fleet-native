//! The Weave config dialog (`WeaveConfigDialog.vue`): the source cards, the file tabs and the code box, Test's and Save's
//! answers, the running folders and the buttons. Sizes are Fleet's Tailwind classes at the 14px root.
use crate::State;
use crate::fleet::settings::harness_weave::{Apply, Flavor};
use crate::fleet::settings::harness_weave_config::{Busy, Check, Config, PROMPTS, folder_name};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{self, Size, Variant, button_lead};
use crate::ui::code_box::code_box_flat;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::{confirm::spinner, rem};
use rune::{Color, Cursor, El, Span, col, el, icon, rich, row, text, text_input};

fn small() -> rune::TextStyle {
    sans(rem(0.75), 400.0, 1.33)
}

/// One of the two source cards (`role="radio"`): chosen has an accent border and wash.
fn source_card(t: &Theme, key: &str, on: bool, disabled: bool, title: &str, sub: El, press: fn(&mut State)) -> El {
    let card = col()
        .key(key.to_string())
        .role("radio")
        .label(title.to_string())
        .checked(on)
        .grow(1.0)
        .min_w(0.0)
        .gap(3.5)
        .px(12.25)
        .py(10.5)
        .radius(RADIUS_BTN)
        .fill(if on { t.accent.alpha(0.1) } else { t.main_bg })
        .border(1.0, if on { t.accent.alpha(0.55) } else { t.border })
        .border_layout()
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(title.to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(sub);
    if disabled { button::disabled(card) } else { card.cursor(Cursor::Pointer).on_press(press) }
}

fn sources(t: &Theme, c: &Config, flavors: &[Flavor], usable: &[Flavor], outdated: &[Flavor]) -> El {
    let own = flavors.iter().map(|f| f.own_path()).collect::<Vec<_>>().join(" \u{b7} ");
    let own_sub = text(own).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full();
    let fleet_words = if !usable.is_empty() {
        "Edited here and tried before it's used. Your own files stay as they are.".to_string()
    } else if !outdated.is_empty() {
        format!("Needs {} or later", outdated.iter().map(|f| format!("{} {}", f.name(), f.minimum())).collect::<Vec<_>>().join(" or "))
    } else {
        "Needs Weave to load first".to_string()
    };
    let fleet_sub = text(fleet_words).font(small()).color(t.muted).wrap().w_full();
    row()
        .key("weave-sources")
        .role("radiogroup")
        .label("Config source")
        .w_full()
        .mt(16.0)
        .gap(10.5)
        .child(source_card(t, "weave-source-own", !c.fleet_source, false, "Use my own file", own_sub, |s: &mut State| {
            s.weave_choose_source(false)
        }))
        .child(source_card(t, "weave-source-fleet", c.fleet_source, usable.is_empty(), "Keep it in Fleet", fleet_sub, |s: &mut State| {
            s.weave_choose_source(true)
        }))
}

/// Own file picked: the box that says Fleet touches nothing (Fleet's border is dashed; Rune has no dashed line).
fn own_box(t: &Theme, c: &Config, usable: &[Flavor]) -> El {
    let mut b = col()
        .key("weave-own-box")
        .w_full()
        .mt(16.0)
        .gap(7.0)
        .px(12.25)
        .py(10.5)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .child(
            text("Weave reads your own files, as it does outside Fleet. Fleet doesn't set anything and doesn't edit them.")
                .font(small())
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    if !usable.is_empty() {
        let copy = button::button(t, Variant::Outline, Size::Sm, None, "Copy into Fleet's config").key("weave-copy-own");
        let copy = if c.busy.is_some() { button::disabled(copy) } else { copy.on_press(|s: &mut State| s.weave_copy_own()) };
        b = b.child(row().flex_wrap().gap(7.0).child(copy));
    }
    b
}

fn tab_font() -> rune::TextStyle {
    mono(11.5, 400.0, 1.43)
}

/// One file's tab: its name, and for a prompt an X that removes it. The active one has `--card-bg` and a 2px accent underline.
fn file_tab(t: &Theme, path: &str, on: bool) -> El {
    let go = path.to_string();
    let name = el()
        .key(format!("weave-tab-{path}"))
        .role("tab")
        .label(path.to_string())
        .px(12.25)
        .py(7.0)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(text(path.to_string()).font(tab_font()).color(if on { t.text } else { t.muted }))
        .on_press(move |s: &mut State| {
            let c = &mut s.settings.harnesses.weave.config;
            c.select(&go);
        });
    let mut tab =
        row().center_y().border_sides([0.0, 1.0, 0.0, 0.0], t.border).fill(if on { t.card_bg } else { Color::TRANSPARENT }).child(name);
    if path.starts_with(PROMPTS) {
        let gone = path.to_string();
        tab = tab.child(
            el().key(format!("weave-remove-{path}"))
                .role("button")
                .label(format!("Remove {path}"))
                .mr(5.25)
                .padding(1.75)
                .radius(4.0)
                .cursor(Cursor::Pointer)
                .child(icon(&X, 12.0).color(t.muted))
                .on_press(move |s: &mut State| s.settings.harnesses.weave.config.remove_prompt(&gone)),
        );
    }
    // The underline sits under the whole tab, X included.
    col().child(tab).child(el().w_full().h(2.0).fill(if on { t.accent } else { Color::TRANSPARENT }))
}

/// The "+ prompt" tab, or the form it turns into.
fn prompt_tab(t: &Theme, c: &Config) -> El {
    if c.adding {
        let input = text_input(&c.new_prompt, mono(11.5, 400.0, 1.43))
            .key("weave-new-prompt")
            .single_line()
            .placeholder("team.md".to_string(), t.muted)
            .label("New prompt file name".to_string())
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w(98.0)
            .h(21.0)
            .px(5.25)
            .radius(4.0)
            .border(1.0, if c.new_prompt.focused { t.accent } else { t.border })
            .fill(t.card_bg)
            .on_edit(|s: &mut State| &mut s.settings.harnesses.weave.config.new_prompt)
            .on_input_key(|s: &mut State, ev: &rune::KeyEvent, _| s.weave_prompt_key(&ev.key))
            .on_submit(|s: &mut State, _| s.settings.harnesses.weave.config.add_prompt());
        let ok = !c.new_prompt.text.trim().is_empty() && c.prompt_problem().is_none();
        let add = button::button(t, Variant::Ghost, Size::Sm, None, "Add").key("weave-prompt-add");
        let add = if ok { add.on_press(|s: &mut State| s.settings.harnesses.weave.config.add_prompt()) } else { button::disabled(add) };
        return row().center_y().gap(3.5).px(7.0).child(text(PROMPTS.to_string()).font(tab_font()).color(t.muted)).child(input).child(add);
    }
    row()
        .key("weave-add-prompt")
        .role("button")
        .label("prompt".to_string())
        .center_y()
        .gap(3.5)
        .px(10.5)
        .cursor(Cursor::Pointer)
        .child(icon(&PLUS, 12.0).color(t.muted))
        .child(text("prompt").font(tab_font()).color(t.muted))
        .on_press(|s: &mut State| s.weave_adding_prompt(true))
}

/// The files of the Fleet source: the strip of tabs over the code box.
fn files(t: &Theme, c: &Config, tabs: &[String], usable: &[Flavor]) -> El {
    let mut strip = row().role("tablist").label("Files").w_full().scroll_x().border_sides([0.0, 0.0, 1.0, 0.0], t.border);
    for path in tabs {
        strip = strip.child(file_tab(t, path, *path == c.active));
    }
    if usable.contains(&Flavor::Weave) {
        strip = strip.child(prompt_tab(t, c));
    }
    col()
        .key("weave-files")
        .w_full()
        .mt(16.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .clip_rounded(RADIUS_BTN)
        .child(strip)
        .child(code_box_flat(
            t,
            "weave-editor",
            &c.active,
            &c.editor,
            |s: &mut State| &mut s.settings.harnesses.weave.config.editor,
            |s: &mut State| s.weave_save(),
        ))
}

/// The hint under the editor, its variables and paths in mono `--text`.
fn hint(t: &Theme, usable: &[Flavor]) -> El {
    let mut words = String::from("Fleet gives this to Weave as ");
    let mut code: Vec<(usize, usize)> = Vec::new();
    let mut put = |words: &mut String, piece: &str, is_code: bool| {
        let at = words.len();
        words.push_str(piece);
        if is_code {
            code.push((at, words.len()));
        }
    };
    for (i, f) in usable.iter().enumerate() {
        if i > 0 {
            put(&mut words, " and ", false);
        }
        put(&mut words, f.variable(), true);
    }
    put(&mut words, " when it starts OpenCode. ", false);
    if usable.contains(&Flavor::Weave) {
        put(&mut words, "Prompt files sit next to it in ", false);
        put(&mut words, "prompts/", true);
        put(&mut words, ", where ", false);
        put(&mut words, "prompt_file", true);
        put(&mut words, " and ", false);
        put(&mut words, "prompt_append_file", true);
        put(&mut words, " look for them. ", false);
    }
    put(&mut words, "Project configs (", false);
    put(&mut words, &usable.iter().map(|f| f.project_path()).collect::<Vec<_>>().join(", "), true);
    put(&mut words, ") still apply on top.", false);
    let spans = code.into_iter().map(|(a, b)| Span::new(a, b).mono().color(t.text)).collect::<Vec<_>>();
    row().key("weave-hint").w_full().mt(7.0).child(rich(words, spans).font(small()).color(t.muted).wrap().shrink().min_w(0.0))
}

/// A harness's answer to Test or Save (`role="status"`).
fn check_card(t: &Theme, n: usize, r: &Check) -> El {
    let (edge, fill, ink) = if r.ok {
        (t.running.alpha(0.3), t.running.alpha(0.1), t.running_text)
    } else {
        (t.error.alpha(0.3), t.error.alpha(0.1), t.error_text)
    };
    let words = if r.ok {
        let n = r.agents.len();
        format!("{} loads {n} {} agent{}", r.harness_name, r.flavor.name(), if n == 1 { "" } else { "s" })
    } else {
        r.error.clone().unwrap_or_default()
    };
    let mut card = col()
        .key(format!("weave-check-{n}"))
        .role("status")
        .w_full()
        .mt(12.0)
        .gap(5.25)
        .px(10.5)
        .py(8.75)
        .radius(RADIUS_CARD)
        .border(1.0, edge)
        .border_layout()
        .fill(fill)
        .child(
            row()
                .gap(5.25)
                .center_y()
                .child(icon(if r.ok { &CHECK } else { &X }, 13.0).color(ink))
                .child(text(words).font(sans(rem(0.75), 600.0, 1.33)).color(ink).wrap().shrink().min_w(0.0)),
        );
    if r.ok {
        let mut pills = row().flex_wrap().gap(3.5);
        for a in &r.agents {
            pills = pills.child(
                row()
                    .px(7.0)
                    .py(0.7)
                    .radius(999.0)
                    .border(1.0, t.border)
                    .border_layout()
                    .fill(t.card_bg)
                    .child(text(a.clone()).font(mono(11.0, 400.0, 1.5)).color(t.text)),
            );
        }
        card = card.child(pills);
    } else {
        if !r.details.is_empty() {
            let mut pre = col().key(format!("weave-check-details-{n}")).scroll_xy();
            for line in r.details.iter().flat_map(|d| d.split('\n')) {
                pre = pre.child(text(line.to_string()).font(mono(11.0, 400.0, 1.5)).color(t.text));
            }
            card = card.child(pre);
        }
        card = card.child(text("Nothing was saved. Sessions keep using the last saved config.").font(small()).color(t.text));
    }
    card
}

/// The running folders a save reaches, and which have reloaded.
fn apply(t: &Theme, a: &Apply) -> El {
    let (n, done) = (a.folders.len(), a.folders.iter().filter(|(_, r)| *r).count());
    let words = if done == n {
        format!("Applied to {}.", if n == 1 { "the running folder".to_string() } else { format!("all {n} running folders") })
    } else {
        format!("Applied to {done} of {n} running folders. The rest reload when their turn finishes.")
    };
    let mut b = col().key("weave-apply").w_full().mt(12.0).gap(5.25).child(text(words).font(small()).color(t.muted).wrap().w_full());
    for (directory, reloaded) in &a.folders {
        let (label, ink) = if *reloaded { ("Reloaded", t.running_text) } else { ("Waiting \u{b7} turn running", t.idle_text) };
        let dot = if *reloaded { t.running } else { t.idle };
        let chip = row()
            .center_y()
            .gap(5.25)
            .px(7.0)
            .py(1.75)
            .radius(999.0)
            .fill(dot.alpha(0.1))
            .border(1.0, dot.alpha(0.35))
            .border_layout()
            .child(el().size(5.25, 5.25).radius(3.0).fill(ink))
            .child(text(label.to_string()).font(sans(11.0, 500.0, 1.5)).color(ink));
        b = b.child(
            row()
                .w_full()
                .center_y()
                .gap(10.5)
                .px(12.25)
                .py(7.0)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.main_bg)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .child(text(folder_name(directory).to_string()).font(sans(12.5, 400.0, 1.5)).color(t.text))
                        .child(text(directory.clone()).font(mono(11.0, 400.0, 1.5)).color(t.muted).truncate().min_w(0.0)),
                )
                .child(chip),
        );
    }
    if let Some(why) = &a.error {
        b = b.child(text(why.clone()).font(small()).color(t.idle_text).wrap().w_full());
    }
    b
}

fn footer(t: &Theme, c: &Config, usable: &[Flavor], saved_fleet: bool, dirty: bool) -> El {
    let busy = c.busy.is_some();
    let off = |b: El| if busy { button::disabled(b) } else { b };
    let mut left = row().gap(3.5);
    if c.fleet_source && !usable.is_empty() {
        let own = usable.iter().map(|f| f.own_path()).collect::<Vec<_>>().join(" and ");
        let copy = button::button(t, Variant::Ghost, Size::Sm, None, &format!("Copy from {own}")).key("weave-copy-from");
        left = left.child(off(copy.on_press(|s: &mut State| s.weave_copy_own())));
    }
    let mut right = row().gap(3.5);
    if c.fleet_source {
        let lead = if c.busy == Some(Busy::Test) { spinner(t.text) } else { icon(&PLAY, rem(1.0)).color(t.text) };
        let test = button_lead(t, Variant::Outline, Size::Sm, Some(lead), "Test").key("weave-test");
        right = right.child(off(test.on_press(|s: &mut State| s.weave_test())));
    }
    let saving = c.busy == Some(Busy::Save);
    let label = if saving && c.fleet_source { "Checking with OpenCode\u{2026}" } else { "Save" };
    let lead = saving.then(|| spinner(t.primary_fg));
    let save = button_lead(t, Variant::Default, Size::Sm, lead, label).key("weave-save");
    let nothing = !dirty && c.fleet_source == saved_fleet;
    let save = if busy || nothing { button::disabled(save) } else { save.on_press(|s: &mut State| s.weave_save()) };
    right = right.child(save);
    row().key("weave-footer").w_full().mt(16.0).flex_wrap().center_y().justify_between().gap(7.0).child(left).child(right)
}

fn body(s: &State, t: &Theme) -> Vec<El> {
    let w = &s.settings.harnesses.weave;
    let (Some(view), c) = (&w.view, &w.config) else { return Vec::new() };
    let flavors = crate::fleet::settings::harness_weave::loaded_flavors(&view.harnesses);
    if flavors.is_empty() {
        return Vec::new();
    }
    let (usable, outdated) = (w.usable(), w.outdated());
    let mut all = vec![sources(t, c, &flavors, &usable, &outdated)];
    if c.fleet_source {
        let tabs = c.file_tabs(&view.harnesses);
        all.push(files(t, c, &tabs, &usable));
        if c.adding
            && let Some(problem) = c.prompt_problem()
        {
            all.push(text(problem.to_string()).key("weave-prompt-problem").font(small()).color(t.idle_text).mt(5.25));
        }
        all.push(hint(t, &usable));
    } else {
        all.push(own_box(t, c, &usable));
    }
    for (n, r) in c.checks.iter().flatten().enumerate() {
        all.push(check_card(t, n, r));
    }
    if let Some(a) = view.apply.as_ref().filter(|a| !a.folders.is_empty()) {
        all.push(apply(t, a));
    }
    if let Some(n) = &c.notice {
        all.push(text(n.clone()).key("weave-notice").role("status").font(small()).color(t.muted).wrap().w_full().mt(12.0));
    }
    if c.fleet_source || w.dirty() {
        all.push(footer(t, c, &usable, view.fleet_source, w.dirty()));
    }
    all
}

/// The dialog and its scrim; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    if !s.settings.harnesses.weave.config.open {
        return None;
    }
    Some(dialog(
        t,
        Dialog {
            key: "weave-config",
            title: "Weave config".into(),
            description: Some("Whose config Weave reads in Fleet sessions.".into()),
            body: body(s, t),
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_weave_config()),
            width: Some(rem(48.0)),
            max_height: Some(s.ui.sel.window.1 * 0.9),
            ..Default::default()
        },
    ))
}
