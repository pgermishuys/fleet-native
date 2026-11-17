//! The two pages no rail owns, drawn over the whole context area (`NO_RAIL`, row C17.19). Ports `components/pages/WelcomePage.vue`
//! and `components/pages/RepositoriesPage.vue`. The Welcome page is static; the Repositories page reads the machine's scan.
use crate::fleet::repositories::Body;
use crate::fleet::store::Route;
use crate::icons::{CIRCLE_ALERT, FOLDER_GIT_2, LOADER_CIRCLE, REFRESH_CW, SETTINGS_2};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{Align, Color, Cycle, El, Shadow, col, el, icon, row, text};

/// `xl:grid-cols-2`: Tailwind's `xl` is 1280px of the 16px browser rem, not 14px-root pixels (Fleet issue 12).
const TWO_COLUMNS_FROM: f32 = 1280.0;

pub fn page(s: &State, t: &Theme) -> El {
    let (key, label, body) = match s.route {
        Route::Welcome => ("welcome-page", "Welcome", welcome(s, t)),
        _ => ("repositories-page", "Repositories", repositories(s, t)),
    };
    let mut page = col()
        .key(key)
        .label(label)
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(12.0)
        .clip_rounded(12.0)
        // `CenterContent.vue`: every page that isn't a session sits in a 24px padded, scrolling body.
        .padding(24.0)
        .child(body);
    for shadow in t.sheet_shadows() {
        page = page.box_shadow(shadow);
    }
    page
}

/// What the build says of itself, for the version line (`v{VITE_APP_VERSION} · {VITE_COMMIT_SHA}`).
pub fn version_text() -> String {
    let build = option_env!("FLEET_NATIVE_BUILD").unwrap_or("dev");
    let commit = option_env!("FLEET_NATIVE_COMMIT").map_or("dev", |c| &c[..c.len().min(7)]);
    format!("v{build} · {commit}")
}

// ─── Welcome ────────────────────────────────────────────────────────────────

/// The logo in its tile, on the Welcome page and the sign-in page alike: `h-18 w-18` (63px) with 10.5px of padding round a
/// `contain` picture; the file is 511×234.
pub fn logo_tile(s: &State, t: &Theme, key: &str) -> El {
    let inner = 42.0;
    let mut logo = el().size(inner, inner * 234.0 / 511.0);
    if let Some(image) = &s.logo {
        logo = logo.pixels(image.pixels());
    }
    el().key(key.to_string())
        .label("Weave logo")
        .size(63.0, 63.0)
        .center()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .box_shadow(Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: 0.0, color: Color::BLACK.alpha(0.2), inset: false })
        .child(logo)
}

fn welcome(s: &State, t: &Theme) -> El {
    let tile = logo_tile(s, t, "welcome-logo");
    let centred = |e: El| e.align(Align::Center);
    let column = col()
        .w(392.0)
        .items_center()
        .gap(14.0)
        .child(tile)
        // `weave-gradient-text` has no CSS in Fleet, so the heading draws in the plain text colour (Fleet issue 1). The heading and
        // "Agent Fleet" share one block, with no gap between them.
        .child(
            col()
                .items_center()
                .child(centred(text("Weave").key("welcome-title").font(sans(26.25, 700.0, 1.2).tracking(-0.025 * 26.25)).color(t.text)))
                .child(centred(text("Agent Fleet").key("welcome-subtitle").font(mono(12.25, 400.0, 1.4)).color(t.muted))),
        )
        .child(centred(
            text("Welcome to Weave Agent Fleet. Select a view from the workspace shell to get started, or open a session to continue recent work.")
                .key("welcome-lede")
                .font(sans(12.25, 400.0, 1.625))
                .color(t.muted)
                .wrap()
                .w_full(),
        ))
        .child(centred(text(version_text()).key("welcome-version").font(sans(10.5, 400.0, 1.4)).color(t.muted)));
    col().w_full().h_full().center().padding(21.0).child(column)
}

// ─── Repositories ───────────────────────────────────────────────────────────

fn repositories(s: &State, t: &Theme) -> El {
    let page = &s.repos_page;
    let busy = page.loading || page.refreshing;
    let refresh = {
        let lead = if page.refreshing { spinner(t, 14.0) } else { icon(&REFRESH_CW, 14.0).color(t.text) };
        let b = button_lead(t, Variant::Outline, Size::Sm, Some(lead), "Refresh").key("repositories-refresh");
        if busy { disabled(b) } else { b.on_press(|s: &mut State| s.read_repositories_page(true)) }
    };
    let settings = button(t, Variant::Outline, Size::Sm, Some(&SETTINGS_2), "Workspace settings")
        .key("repositories-settings")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.open_workspace_settings(tx));
    let header = row()
        .key("repositories-header")
        .w_full()
        .flex_wrap()
        .gap(8.0)
        .items_center()
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text("Repositories").key("repositories-title").font(sans(21.0, 600.0, 1.25).tracking(-0.025 * 21.0)).color(t.text))
                .child(
                    text("Git repositories Fleet found in your locations (Settings → Folders).")
                        .font(sans(12.25, 400.0, 1.5))
                        .color(t.muted)
                        .wrap()
                        .mt(4.0),
                ),
        )
        .child(el().grow(1.0))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text(s.repositories_last_scan()).key("repositories-last-scan").font(sans(10.5, 400.0, 1.4)).color(t.muted))
                .child(refresh)
                .child(settings),
        );
    let body = match page.body() {
        Body::Loading => loading(t),
        Body::Error => error_note(t, page.error.as_deref().unwrap_or_default()),
        Body::Empty => empty(t),
        Body::List => cards(s, t),
    };
    col()
        .key("repositories-scroll")
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(21.0)
        .gap(21.0)
        .scroll(rune::ScrollSpec { stick_bottom: false })
        .child(header)
        .child(body)
}

fn spinner(t: &Theme, size: f32) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, size).color(t.muted).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// The first read: a bordered box, square-cornered as Fleet draws it.
fn loading(t: &Theme) -> El {
    row()
        .key("repositories-loading")
        .role("status")
        .w_full()
        .grow(1.0)
        .center()
        .gap(10.5)
        .padding(28.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(spinner(t, 20.0))
        .child(text("Loading repositories…").font(sans(12.25, 400.0, 1.4)).color(t.muted))
}

/// `ErrorNote` with no way to try again: the user presses Refresh.
fn error_note(t: &Theme, reason: &str) -> El {
    let font = |w| sans(rem(0.875), w, 1.43);
    row()
        .key("repositories-error")
        .role("alert")
        .w_full()
        .gap(rem(0.5))
        .items_start()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(
            col()
                .shrink()
                .min_w(0.0)
                .child(text("Unable to load repositories").font(font(500.0)).color(t.error_text).wrap())
                .child(text(reason.to_string()).font(font(400.0)).color(t.error_text).wrap().mt(2.0)),
        )
}

/// `EmptyState bordered`: a dashed box with the icon, the words and the way to Settings.
fn empty(t: &Theme) -> El {
    col()
        .key("repositories-empty")
        .w_full()
        .grow(1.0)
        .center()
        .gap(3.5)
        .padding(21.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(icon(&FOLDER_GIT_2, 28.0).color(t.muted).mb(7.0))
        .child(text("No repositories found").font(sans(14.0, 600.0, 1.4)).color(t.text))
        .child(
            text("Add a location in Settings → Folders, and Fleet lists the repositories inside it.")
                .font(sans(12.25, 400.0, 1.5))
                .color(t.muted)
                .wrap()
                .align(Align::Center)
                .max_w(455.0),
        )
        .child(
            row().gap(7.0).mt(10.5).child(
                button(t, Variant::Outline, Size::Sm, Some(&SETTINGS_2), "Open settings")
                    .key("repositories-open-settings")
                    .on_press_tx(|s: &mut State, tx: &mut Tx| s.open_workspace_settings(tx)),
            ),
        )
}

fn cards(s: &State, t: &Theme) -> El {
    let columns = if s.ui.sel.window.0 >= TWO_COLUMNS_FROM { 2 } else { 1 };
    let mut cards = s.repos_page.sorted().into_iter().enumerate().map(|(n, r)| card(t, n, &r.name, &r.path, &r.parent_root));
    let mut grid = col().key("repositories-list").w_full().gap(14.0);
    loop {
        let mut line = row().w_full().gap(14.0).items_start();
        let mut filled = 0;
        for card in cards.by_ref().take(columns) {
            line = line.child(card);
            filled += 1;
        }
        if filled == 0 {
            break;
        }
        for _ in filled..columns {
            line = line.child(el().grow(1.0).w(0.0));
        }
        grid = grid.child(line);
    }
    grid
}

/// One repository: a read-only `Card` (`gap-4 py-5`, `px-5` inside).
fn card(t: &Theme, n: usize, name: &str, path: &str, root: &str) -> El {
    let tile =
        el().padding(7.0).border(1.0, t.border).border_layout().fill(t.muted.alpha(0.3)).child(icon(&FOLDER_GIT_2, 16.0).color(t.muted));
    let heading = row().px(17.5).gap(10.5).items_start().child(tile).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .child(text(name.to_string()).key(format!("repository-name-{n}")).font(sans(14.0, 600.0, 1.0)).color(t.text).truncate())
            .child(
                text(path.to_string()).key(format!("repository-path-{n}")).font(sans(10.5, 400.0, 1.4)).color(t.muted).truncate().mt(3.5),
            ),
    );
    let detail = row().px(17.5).gap(7.0).items_start().child(text("Workspace root").font(sans(12.25, 400.0, 1.4)).color(t.muted)).child(
        text(root.to_string()).key(format!("repository-root-{n}")).font(sans(12.25, 400.0, 1.4)).color(t.text).wrap().min_w(0.0).grow(1.0),
    );
    col()
        .key(format!("repository-{n}"))
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .py(17.5)
        .gap(14.0)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .box_shadow(Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Color::BLACK.alpha(0.05), inset: false })
        .child(heading)
        .child(detail)
}
