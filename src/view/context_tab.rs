//! The Context tab (`SessionContextCanvas.vue`, rows 12.2 to 12.9 of docs/parity/08-session-header-and-context.md): the
//! GitHub notice, the sections of links, loading, the error, the empty note, the attach form, the workspace list and the
//! footer with Refresh. What is in each section, and every sentence, is worked out in `fleet::context_tab`. Pull requests
//! draw as cards (`smart_link_card`), issues as rows (`smart_link_row`).
use super::smart_link_card::smart_link_card;
use super::smart_link_row::{flash, link_row};
use crate::State;
use crate::fleet::context_tab::{Feedback, Groups, automation_detail, automation_origin, feedback, own_heading, workspace_rows};
use crate::fleet::smart_links::{Enrichment, format_checked_ago};
use crate::icons::{CIRCLE_ALERT, GIT_BRANCH, LOADER_CIRCLE, PAPERCLIP, PLUS, REFRESH_CW, ZAP};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use rune::{Cursor, Cycle, El, Key, KeyEvent, ScrollSpec, Tx, col, el, icon, row, text, text_input};

/// `.context-link-btn`: accent text, 12px/500, a faint wash on hover.
fn link_btn(t: &Theme, key: &str, glyph: Option<El>, label: &str) -> El {
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .gap(5.0)
        .px(4.0)
        .py(2.0)
        .center_y()
        .radius(5.0)
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .focus_ring(2.0, 1.0, t.accent);
    if let Some(glyph) = glyph {
        b = b.child(glyph);
    }
    b.child(text(label.to_string()).font(sans(12.0, 500.0, 1.3)).color(t.accent_text))
}

fn spinner(ink: rune::Color, size: f32) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, size).color(ink).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `.context-section__label`: 10.5px, 600, uppercase, tracked.
fn heading(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_uppercase()).key(key.to_string()).role("heading").font(sans(10.5, 600.0, 1.3).tracking(0.05 * 10.5)).color(t.muted)
}

fn section(key: &str, label: &str, t: &Theme, body: Vec<El>) -> El {
    col().key(key.to_string()).label(label.to_string()).w_full().gap(6.0).child(heading(t, &format!("{key}-label"), label)).children(body)
}

pub fn context_tab(s: &State, t: &Theme) -> El {
    let session = s.session();
    let links = s.context_links();
    let groups = Groups::of(&links);
    let records = s.smart_links.origins.get(&session.id).map(Vec::as_slice).unwrap_or_default();
    let automation = automation_origin(session.row.origin.as_ref(), records);
    let now = super::message::now_ms(s);
    let offset = crate::conv::time::local_offset_secs();
    let has_links = !links.is_empty();
    let mut body = col().w_full().gap(18.0).padding(14.0).pt(12.0).pb(14.0).items_stretch();

    if links.iter().any(|l| l.enrichment == Enrichment::NotConnected) {
        body = body.child(notice(t));
    }
    if groups.origin.is_some() || automation.is_some() {
        let inner = if let Some(a) = &automation {
            let origin = row()
                .key("context-origin")
                .ml(-6.0)
                .mr(-6.0)
                .padding(6.0)
                .gap(8.0)
                .items_start()
                .radius(RADIUS_BTN)
                .child(row().w(16.0).mt(2.0).child(icon(&ZAP, 14.0).color(t.status_waiting())))
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .min_w(0.0)
                        .gap(2.0)
                        .child(text(a.name.clone()).font(sans(13.0, 500.0, 1.3)).color(t.text).wrap())
                        .child(text(automation_detail(a, now, offset)).font(sans(11.5, 400.0, 1.3)).color(t.muted).wrap()),
                );
            flash(origin, t, s, "origin")
        } else if let Some(link) = groups.origin {
            let inner = if crate::fleet::smart_links::is_pull_request(link) {
                smart_link_card(s, t, link)
            } else {
                link_row(s, t, link, Some("Session started here"))
            };
            col().key("context-origin").w_full().child(inner)
        } else {
            el()
        };
        body = body.child(section("context-started-from", "Started from", t, vec![inner]));
    }
    if !groups.own.is_empty() {
        let rows = groups.own.iter().map(|l| smart_link_card(s, t, l)).collect();
        body = body.child(section("context-own", own_heading(groups.own.len()), t, rows));
    }
    if !groups.pinned.is_empty() {
        let rows = groups
            .pinned
            .iter()
            .map(|l| if crate::fleet::smart_links::is_pull_request(l) { smart_link_card(s, t, l) } else { link_row(s, t, l, None) })
            .collect();
        body = body.child(section("context-pinned", "Pinned", t, rows));
    }
    if !groups.mentioned.is_empty() {
        let rows = groups.mentioned.iter().map(|l| link_row(s, t, l, None)).collect();
        body = body.child(section("context-mentioned", "Mentioned in conversation", t, rows));
    }

    let loading = s.smart_links.is_loading(&session.id);
    let error = if has_links { None } else { s.smart_links.load_error(&session.id) };
    match feedback(has_links, automation.is_some(), loading, error) {
        Feedback::Loading => {
            body = body.child(
                row()
                    .key("context-links-loading")
                    .role("status")
                    .label("Loading what's attached\u{2026}")
                    .self_center()
                    .my(12.0)
                    .child(spinner(t.muted, 20.0)),
            )
        }
        Feedback::Error(why) => body = body.child(error_note(t, &why)),
        Feedback::Empty => body = body.child(empty(t)),
        Feedback::None => {}
    }

    body = body.child(attach(s, t));
    let rows = workspace_rows(
        Some(s.header_model().branch.as_deref().unwrap_or("")),
        session.row.isolation.as_deref(),
        &s.header_model().directory,
    );
    if !rows.is_empty() {
        body = body.child(workspace(t, rows));
    }
    if has_links {
        body = body.child(el().grow(1.0).min_h(0.0)).child(footer(s, t, now, offset));
    }
    col()
        .key("context")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("context-scroll").w_full().grow(1.0).min_h(0.0))
}

/// `.context-notice`: Fleet's "Connect GitHub" nudge, once however many links can't be checked.
fn notice(t: &Theme) -> El {
    row()
        .key("context-notice")
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(6.0)
        .px(12.0)
        .py(9.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.status_waiting().alpha(0.08))
        .child(text("Connect GitHub to see status, checks and reviews.").font(sans(12.5, 400.0, 1.4)).color(t.text).wrap())
        .child(link_btn(t, "context-connect", None, "Connect GitHub").on_press(|s: &mut State| s.connect_github()))
}

/// `EmptyState compact` with the paperclip.
fn empty(t: &Theme) -> El {
    col()
        .key("context-empty")
        .w_full()
        .items_center()
        .gap(4.0)
        .px(12.0)
        .py(14.0)
        .child(icon(&PAPERCLIP, 20.0).color(t.muted).mb(8.0))
        .child(text("Nothing attached yet").font(sans(12.25, 500.0, 1.4)).color(t.text))
        .child(
            text("Pull requests and issues mentioned in the conversation, and pull requests this session opens, show up here.")
                .font(sans(10.5, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .w_full()
                .align(rune::Align::Center),
        )
}

/// `ErrorNote` with a reason and Try again.
fn error_note(t: &Theme, why: &str) -> El {
    let retry =
        button(t, Variant::Outline, Size::Sm, None, "Try again").key("context-retry").on_press(|s: &mut State| s.retry_links()).mt(8.0);
    row()
        .key("context-error")
        .role("alert")
        .w_full()
        .gap(8.0)
        .items_start()
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .grow(1.0)
                .min_w(0.0)
                .child(text("Couldn't load what's attached to this session.").font(sans(12.25, 500.0, 1.43)).color(t.error_text).wrap())
                .child(text(why.to_string()).font(sans(12.25, 400.0, 1.43)).color(t.error_text).wrap().mt(2.0))
                .child(row().child(retry)),
        )
}

/// The attach button, or its form.
fn attach(s: &State, t: &Theme) -> El {
    let c = &s.context;
    if !c.attaching {
        return row().key("context-attach").w_full().child(
            link_btn(t, "context-attach-start", Some(icon(&PLUS, 13.0).color(t.accent_text)), "Attach a pull request or issue")
                .ml(-4.0)
                .on_press(|s: &mut State| s.start_attach()),
        );
    }
    let field = text_input(&c.url, mono(11.5, 400.0, 1.3))
        .key("context-attach-url")
        .single_line()
        .placeholder("https://github.com/owner/repo/pull/123".to_string(), t.muted)
        .label("Attach a pull request or issue".to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(30.0)
        .px(9.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .focus_ring(2.0, 1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.context.url)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Escape => {
                s.context.cancel_attach();
                true
            }
            Key::Enter => {
                s.submit_attach();
                true
            }
            _ => false,
        });
    let can = c.can_attach();
    let mut submit = row()
        .key("context-attach-submit")
        .role("button")
        .label("Attach")
        .h(26.0)
        .px(12.0)
        .center()
        .radius(RADIUS_BTN)
        .fill(t.accent)
        .child(text("Attach").font(sans(12.0, 500.0, 1.3)).color(t.primary_fg));
    submit =
        if can { submit.cursor(Cursor::Pointer).on_press(|s: &mut State| s.submit_attach()) } else { submit.opacity(0.5).disabled(true) };
    let mut form = col()
        .key("context-attach-form")
        .w_full()
        .gap(6.0)
        .child(text("Attach a pull request or issue").font(sans(11.5, 400.0, 1.3)).color(t.muted))
        .child(field)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(submit)
                .child(link_btn(t, "context-attach-cancel", None, "Cancel").on_press(|s: &mut State| s.context.cancel_attach())),
        );
    if let Some(why) = &c.attach_error {
        form =
            form.child(text(why.clone()).key("context-attach-error").role("alert").font(sans(11.5, 400.0, 1.3)).color(t.error_text).wrap());
    }
    form
}

/// The Workspace section: a two-column list.
fn workspace(t: &Theme, rows: Vec<(&'static str, String)>) -> El {
    let mut list = col().w_full().gap(4.0);
    for (name, value) in rows {
        let mut v = row().grow(1.0).shrink().min_w(0.0).gap(4.0).center_y();
        if name == "Branch" {
            v = v.child(icon(&GIT_BRANCH, 12.0).color(t.text));
        }
        v = v.child(text(value.clone()).font(mono(11.5, 400.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0));
        list = list.child(
            row()
                .key(format!("context-workspace-{}", name.to_lowercase()))
                .w_full()
                .gap(10.0)
                .center_y()
                .help(value)
                .child(text(name.to_string()).font(sans(12.0, 400.0, 1.3)).color(t.muted).w(72.0))
                .child(v),
        );
    }
    section("context-workspace", "Workspace", t, vec![list])
}

/// "GitHub checked 4 minutes ago" (or "Waiting for GitHub") and Refresh.
fn footer(s: &State, t: &Theme, now: i64, offset: i64) -> El {
    let checked = s.smart_links.last_checked_at(&s.session().id);
    let words = match format_checked_ago(checked.as_deref(), now, offset) {
        Some(ago) => format!("GitHub checked {ago}"),
        None => "Waiting for GitHub".to_string(),
    };
    let busy = s.context.refreshing(s.clock);
    let lead = if busy { spinner(t.accent_text, 12.0) } else { icon(&REFRESH_CW, 12.0).color(t.accent_text) };
    let mut refresh = link_btn(t, "context-refresh", Some(lead), "Refresh");
    refresh = if busy {
        refresh.opacity(0.7).disabled(true).cursor(Cursor::Default)
    } else {
        refresh.on_press(|s: &mut State| s.refresh_context())
    };
    row()
        .key("context-footer")
        .w_full()
        .center_y()
        .gap(8.0)
        .pt(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(text(words).key("context-checked").font(sans(11.5, 400.0, 1.3)).color(t.muted).grow(1.0))
        .child(refresh)
}
