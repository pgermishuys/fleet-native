//! The Automations page (component 19): `[automations-list][automations-detail]`. Ports `components/pages/AutomationsPage.vue`
//! and `AutomationDetailPanel.vue`. The list is `list`; an open automation's header, actions and note are `detail`; the
//! composer under a new or an open automation is `composer`.
use crate::State;
use crate::fleet::automations::nav::{RETRYING, ViewMode};
use crate::icons::{CIRCLE_ALERT, PLUS, ZAP};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::{empty_state, page_header, rem};
use crate::view::{files_tab, workflows::sheet};
use rune::{ScrollSpec, col, icon, row, text};

pub mod builtin;
pub mod composer;
pub mod detail;
pub mod list;
pub mod runs;
pub mod strip;
pub mod when;

/// The whole context: the list, its gutter and the detail; with the panel collapsed, the detail alone.
pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    if s.context_panel_shown() { vec![list::panel(s, t), super::gutter(t), detail(s, t)] } else { vec![detail(s, t)] }
}

/// Whether Try again is running, and what it does.
pub(super) type Retry = (bool, fn(&mut State, &mut rune::Tx));

/// `ErrorNote`: the words, the server's reason under them, and Try again (disabled while it runs) when `retry` is given.
pub(super) fn error_note(t: &Theme, key: &str, message: &str, reason: Option<&str>, retry: Option<Retry>) -> rune::El {
    let mut body = col().grow(1.0).shrink().min_w(0.0).child(
        text(message.to_string()).font(sans(rem(0.875), if reason.is_some() { 500.0 } else { 400.0 }, 1.43)).color(t.error_text).wrap(),
    );
    if let Some(reason) = reason {
        body = body.child(text(reason.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.error_text).wrap().mt(2.0));
    }
    if let Some((retrying, go)) = retry {
        let again = button(t, Variant::Outline, Size::Sm, None, "Try again").key(format!("{key}-retry")).on_press_tx(go);
        body = body.child(row().mt(8.0).child(if retrying { disabled(again) } else { again }));
    }
    row()
        .key(key.to_string())
        .role("alert")
        .gap(8.0)
        .items_start()
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(body)
}

/// `AutomationDetailPanel`: the bar with the title, and the body under it.
fn detail(s: &State, t: &Theme) -> rune::El {
    let nav = &s.automations_nav;
    let store = s.shown_automations();
    let open = nav.open_id().and_then(|id| store.get(id));
    let title = match (nav.mode, open) {
        (ViewMode::Create, _) => "New automation".to_string(),
        (ViewMode::Edit, Some(a)) => a.name.clone(),
        _ => "Automations".to_string(),
    };
    let header = match (nav.mode, open) {
        (ViewMode::Edit, Some(a)) if a.builtin.is_some() => builtin::header(t, a),
        (ViewMode::Edit, Some(a)) => detail::header(s, t, a),
        (ViewMode::Create, _) => {
            page_header::bar_with(t, "automation-header", &title, vec![builtin::pill(t, "automation-not-saved", "Not saved")], Vec::new())
        }
        _ => page_header::bar(t, "automation-header", &title, None),
    };
    // A new automation, and one of yours open: the composer under the body, the form error over it (a built-in has none).
    let composes = match (nav.mode, open) {
        (ViewMode::Create, _) => true,
        (ViewMode::Edit, Some(a)) => a.builtin.is_none(),
        _ => false,
    };
    // `center__body`: the page sits in 24 px of padding all round, and its own body scrolls under the bar.
    let page = col().w_full().grow(1.0).min_h(0.0).padding(24.0).child(header).child(
        col()
            .key("automation-body")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .px(24.0)
            .pt(18.0)
            .pb(8.0)
            .child(body(s, t))
            .scroll(ScrollSpec { stick_bottom: false }),
    );
    let page = if composes {
        page.children(nav.form_error.as_deref().map(|words| detail::form_error(t, words))).child(composer::section(s, t))
    } else {
        page
    };
    sheet(t, "automations-detail").child(page)
}

/// What the list mode's body says: loading, a failed load, or the empty state (rows 3.3 to 3.5).
fn body(s: &State, t: &Theme) -> rune::El {
    let store = s.shown_automations();
    // The open automation's page; one the list doesn't hold (yet) is the list mode's body. A new automation's is piece H's.
    let open = s.automations_nav.open_id().and_then(|id| store.get(id));
    if let (Some(a), ViewMode::Edit) = (open, s.automations_nav.mode) {
        return if a.builtin.is_some() { builtin::body(t, s, a) } else { detail::body(t, s, a) };
    }
    if s.automations_nav.mode == ViewMode::Create {
        return create_prompt(t);
    }
    let first_load = store.is_loading && store.list.is_empty();
    let failed = store.list.is_empty() && (store.error.is_some() || s.automations_nav.retrying);
    let centred = col().w_full().grow(1.0).center();
    if first_load {
        return centred
            .child(row().key("automation-loading").label("Loading automations…").child(files_tab::spinner(t.muted, rem(1.4286))));
    }
    if failed {
        let reason = store.error.as_deref().unwrap_or(RETRYING);
        let note = error_note(
            t,
            "automation-load-error",
            "Couldn't load automations.",
            Some(reason),
            Some((s.automations_nav.retrying, |s, tx| s.automations_retry(tx))),
        );
        return centred.child(col().w_full().max_w(rem(28.0)).child(note));
    }
    let none = store.list.is_empty();
    let (title, words) = if none {
        ("No automations yet", "An automation runs a prompt or a workflow on a schedule, or when something happens in Fleet.")
    } else {
        ("Automations", "Pick one from the list, or make a new one.")
    };
    let action = none.then(|| {
        button(t, Variant::Default, Size::Sm, Some(&PLUS), "New automation")
            .key("automation-empty-create")
            .on_press_tx(|s: &mut State, tx: &mut rune::Tx| s.start_automation_create(tx))
    });
    centred.child(empty_state::full(t, "automation-empty", &ZAP, title, words, action))
}

/// `.automation-page__empty`: what a new automation's page says over the composer, centred in the body.
fn create_prompt(t: &Theme) -> rune::El {
    col().key("automation-create-prompt").w_full().grow(1.0).center().child(
        col()
            .w_full()
            .max_w(460.0)
            .items_center()
            .child(
                text("What should run on its own?")
                    .font(sans(17.0, 600.0, 1.4).tracking(-0.17))
                    .color(t.text)
                    .wrap()
                    .w_full()
                    .align(rune::Align::Center)
                    .mb(4.0),
            )
            .child(
                text("Say what it should do and when. The chips under the box say where it runs.")
                    .font(sans(13.5, 400.0, 1.5))
                    .color(t.muted)
                    .wrap()
                    .w_full()
                    .align(rune::Align::Center),
            ),
    )
}
