//! Updates & about › Fleet (`SystemSection.vue`): the installed version, the app's own row, and this Fleet's server row (its
//! status, "Download", "Check for updates", the bar and its notes). Everything here is about the machine Settings shows.
use super::kit;
use super::updates::{bar, check_button, error_row, ink, link, link_in, note, pill, status_icon};
use crate::State;
use crate::fleet::settings::updates_page as words;
use crate::fleet::updates::{Status, UpdateStatus};
use crate::theme::{Theme, mono, sans};
use crate::ui::rem;
use rune::{El, col, row, text};

pub fn group(s: &State, t: &Theme) -> El {
    let key = s.settings.choice.key(&s.machines).to_string();
    let store = s.updates.status.get(&key);
    let status = store.and_then(|st| st.status.as_ref());
    let busy = store.is_some_and(|st| st.loading || st.checking);
    let mut rows = vec![version_row(s, t, status)];
    // The app is this window: its row never follows the machine switch, and it is always here (native is always "in the app").
    rows.push(super::updates_app::view(s, t));
    // A Fleet the app manages is updated with it, so the server's row is left out (`showServerUpdates`).
    if status.map(|st| st.status) != Some(Status::Managed) {
        rows.extend(server_rows(t, &key, status, busy));
    }
    kit::group(t, "fleet", "Fleet", Some("This Fleet's version and its updates."), rows)
}

/// "Installed version", and under it the version in mono, "Loading…" until the Fleet answers. While an update waits the line
/// points at Machines.
fn version_row(s: &State, t: &Theme, status: Option<&UpdateStatus>) -> El {
    let small = sans(rem(0.75), 400.0, 1.33);
    let below = match words::version_text(status) {
        Some(version) => {
            let mut line = row()
                .key("fleet-version-line")
                .mt(2.0)
                .flex_wrap()
                .gap(4.0)
                .child(text(version).key("fleet-version-text").font(mono(rem(0.75), 400.0, 1.33)).color(t.muted));
            if words::update_waiting(status) {
                line = line.child(link_in(t, "system-see-machines", "see Machines", mono(rem(0.75), 400.0, 1.33), |s: &mut State| {
                    s.open_settings("updates", Some("machines"))
                }));
            }
            line
        }
        None => row().mt(2.0).child(text("Loading\u{2026}").key("fleet-version-loading").font(small).color(t.muted)),
    };
    kit::Row::new("fleet-version", "Installed version").below(below).build(s, t)
}

fn server_rows(t: &Theme, key: &str, status: Option<&UpdateStatus>, busy: bool) -> Vec<El> {
    let tone = words::server_tone(status);
    let downloading = status.map(|st| st.status) == Some(Status::Downloading);
    let waiting = busy || downloading;
    let mut left = row().center_y().gap(7.0);
    if let Some(i) = status_icon(t, tone, waiting) {
        left = left.child(i);
    }
    left = left.child(text(words::server_label(status)).key("fleet-update-status").font(sans(rem(0.875), 400.0, 1.43)).color(ink(t, tone)));
    let mut right = row().center_y().gap(7.0);
    if words::notes_version(status).is_some() {
        right = right.child(link(t, "fleet-whats-new", "What's new", |s: &mut State| s.open_settings("updates", Some("whats-new"))));
    }
    if words::can_download(status) {
        let k = key.to_string();
        right = right.child(
            pill(t, "fleet-update-download", "Download", None, false)
                .on_press(move |s: &mut State| s.update_machine(&k, "/api/update/download")),
        );
    }
    if words::can_check(status) {
        let k = key.to_string();
        right = right.child(check_button(t, "fleet-update-check", waiting, move |s: &mut State| s.update_machine(&k, "/api/update/check")));
    }
    let mut body =
        col().w_full().px(14.0).py(10.5).gap(7.0).child(row().w_full().items_center().justify_between().gap(10.5).child(left).child(right));
    if let Some((percent, label)) = words::download_progress(status) {
        let mut b = col().w_full().gap(3.5).child(bar(t, "fleet-update-bar", percent));
        if let Some(l) = label {
            b = b.child(note(t, "fleet-update-progress", &l));
        }
        body = body.child(b);
    }
    let mut rows = vec![body.key("fleet-update-row")];
    if let Some(why) = status.filter(|st| st.status == Status::Error).and_then(|st| st.error.as_deref()).filter(|e| !e.is_empty()) {
        rows.push(error_row(t, "fleet-update-error", why));
    }
    if status.map(|st| st.status) == Some(Status::Staged) {
        rows.push(
            row()
                .key("fleet-update-staged")
                .w_full()
                .px(14.0)
                .py(10.5)
                .flex_wrap()
                .child(
                    text("It's downloaded and installs the next time you start Fleet (").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted),
                )
                .child(text("fleet").font(mono(rem(0.75), 400.0, 1.33)).color(t.muted))
                .child(text(").").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)),
        );
    }
    rows
}
