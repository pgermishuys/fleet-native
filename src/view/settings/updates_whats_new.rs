//! Updates & about › What's new (`WhatsNewSection.vue`): the release notes in a card, every version at or above the installed one
//! open and a few older ones folded, each with its badge and date. The notes are the hand-written intro as Markdown and then
//! New and Fixed lists drawn here, so a change's part of Fleet, its title and its pull request can each look as Fleet's CSS
//! makes them. What the rows say and which are open is `fleet::updates::whats_new`.
use crate::State;
use crate::fleet::updates::notes::{ParsedReleaseNotes, ReleaseChange, ReleaseNote, intro_markdown, parse_release_notes, whats_new_range};
use crate::fleet::updates::whats_new::{self as words, ALL_RELEASES_URL, Badge, FLASH_SECS};
use crate::icons::{CHEVRON_RIGHT, EXTERNAL_LINK};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use crate::view::prose;
use rune::{Cursor, El, Link, LinkPaint, Span, Tx, col, el, icon, rich, row, text};
use rune_kit::markdown::Markdown;
use std::f32::consts::FRAC_PI_2;

fn open_url(url: String) -> impl Fn(&mut State, &mut Tx) + Send + Sync + 'static {
    move |_: &mut State, tx: &mut Tx| {
        let _ = tx.open_url(&url);
    }
}

pub fn group(s: &State, t: &Theme) -> El {
    let machine = s.settings.choice.key(&s.machines).to_string();
    let status = s.updates.status.get(&machine).and_then(|st| st.status.as_ref());
    let installed = status.map(|st| st.current_version.as_str()).filter(|v| !v.is_empty());
    let ready = status.filter(|st| st.is_staged()).and_then(|st| st.latest_version.as_deref());
    let notes = s.updates.releases.get(&machine);
    let releases = notes.map(|n| n.releases.as_slice()).unwrap_or_default();
    let (loading, error) = (notes.is_none_or(|n| n.loading || !n.loaded), notes.and_then(|n| n.error.as_deref()));
    let body = if releases.is_empty() {
        empty(t, loading, error)
    } else {
        let mut list = col().w_full();
        if let Some(e) = error {
            let saved = words::day(
                notes.and_then(|n| n.fetched_at.as_deref()),
                true,
                crate::view::message::now_ms(s),
                crate::conv::time::local_offset_secs(),
            );
            list = list.child(
                text(words::stale_line(e, &saved))
                    .key("whats-new-stale")
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted)
                    .wrap()
                    .w_full()
                    .mb(7.0),
            );
        }
        let range = whats_new_range(releases, installed);
        for (at, release) in range.open.iter().chain(range.folded.iter()).enumerate() {
            let open = s.updates.whats_new.opened.get(&release.version).copied().unwrap_or(at < range.open.len());
            list = list.child(version(s, t, release, at == 0, open, words::badge(&release.version, installed, ready)));
        }
        list
    };
    let all = row()
        .key("whats-new-all")
        .role("link")
        .label("All releases on GitHub")
        .center_y()
        .gap(3.5)
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press_tx(open_url(ALL_RELEASES_URL.into()))
        .child(
            rich("All releases on GitHub".to_string(), vec![Span::new(0, "All releases on GitHub".len()).underline(t.muted)])
                .font(sans(rem(0.75), 400.0, 1.33))
                .color(t.muted)
                .group_color(t.text),
        )
        .child(icon(&EXTERNAL_LINK, 12.0).color(t.muted).group_color(t.text));
    let head = row()
        .w_full()
        .items_end()
        .justify_between()
        .gap(14.0)
        .px(3.5)
        .mb(7.0)
        .child(text("What's new").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(all);
    let card = col()
        .key("whats-new-card")
        .w_full()
        .px(14.0)
        .py(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_CARD)
        .child(body);
    col().key("settings-group-whats-new").w_full().child(head).child(card)
}

/// Loading, or nothing at all: the reason and where the notes are.
fn empty(t: &Theme, loading: bool, error: Option<&str>) -> El {
    let font = sans(rem(0.875), 400.0, 1.43);
    if loading {
        return text("Loading release notes\u{2026}").key("whats-new-loading").font(font).color(t.muted);
    }
    let mut words = format!("{} They're on ", words::empty_line(error));
    let start = words.len();
    words.push_str("GitHub");
    let range = start..words.len();
    words.push('.');
    rich(words, vec![Span::new(range.start, range.end).color(t.muted).underline(t.muted)])
        .key("whats-new-empty")
        .font(font)
        .color(t.muted)
        .wrap()
        .w_full()
        .link(
            Link::new(range, ALL_RELEASES_URL.to_string())
                .hover(LinkPaint::new().color(t.text))
                .focused(LinkPaint::new().ring(t.accent, 2.0)),
        )
        .on_link(|_: &mut State, url: &str, tx: &mut Tx| {
            let _ = tx.open_url(url);
        })
}

fn badge(t: &Theme, b: Badge) -> El {
    let (border, fill, ink) =
        if b.accent() { (t.accent.alpha(0.3), t.accent.alpha(0.1), t.accent_text) } else { (t.border, t.card_bg.alpha(0.0), t.muted) };
    row()
        .key(format!("whats-new-badge-{}", b.label().to_lowercase().replace(' ', "-")))
        .px(8.0)
        .py(1.0)
        .radius(999.0)
        .border(1.0, border)
        .border_layout()
        .fill(fill)
        .child(text(b.label()).font(sans(10.0, 500.0, 1.4)).color(ink))
}

/// One version's row (a `details`): the summary line, and when it is open the notes under it.
fn version(s: &State, t: &Theme, release: &ReleaseNote, first: bool, open: bool, b: Option<Badge>) -> El {
    let v = release.version.clone();
    let now = crate::view::message::now_ms(s);
    let date = words::day(release.published_at.as_deref(), true, now, crate::conv::time::local_offset_secs());
    let toggle = v.clone();
    let mut summary = row()
        .key(format!("whats-new-summary-{v}"))
        .role("button")
        .label(format!("Fleet {v}"))
        .w_full()
        .center_y()
        .gap(10.0)
        .py(12.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press(move |s: &mut State| s.toggle_whats_new_row(&toggle, open))
        .child(icon(&CHEVRON_RIGHT, 14.0).color(t.muted).rotate(if open { FRAC_PI_2 } else { 0.0 }))
        .child(text(format!("Fleet {v}")).font(sans(rem(0.875), 600.0, 1.43)).color(t.text));
    if let Some(b) = b {
        summary = summary.child(badge(t, b));
    }
    summary = summary.child(el().grow(1.0)).child(text(date).font(crate::theme::mono(rem(0.75), 400.0, 1.33)).color(t.muted));
    // The row a link named flashes: accent at 9% for 40% of the time, then fading out.
    let fill = match &s.updates.whats_new.flash {
        Some((flashed, at)) if *flashed == v => {
            let k = ((s.clock - at) / FLASH_SECS).clamp(0.0, 1.0) as f32;
            let fade = if k < 0.4 { 1.0 } else { 1.0 - (k - 0.4) / 0.6 };
            t.accent.alpha(0.09 * fade)
        }
        _ => t.accent.alpha(0.0),
    };
    let mut inner = col().w_full().radius(8.0).fill(fill).child(summary);
    if open {
        let parsed = parse_release_notes(&release.body);
        inner = inner.child(notes(t, release, &parsed));
    }
    let mut outer = col().key(words_key(&v)).w_full();
    if !first {
        outer = outer.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    outer.child(inner)
}

fn words_key(v: &str) -> String {
    words::row_key(v)
}

/// The notes (`.whats-new__notes`, indent 24, 13.5 px) and the behind-the-scenes line.
fn notes(t: &Theme, release: &ReleaseNote, parsed: &ParsedReleaseNotes) -> El {
    let v = &release.version;
    let mut body = col().key(format!("whats-new-notes-{v}")).w_full().pl(24.0).pb(16.0);
    let intro = intro_markdown(parsed);
    if !intro.trim().is_empty() {
        body = body.child(
            Markdown::new(&intro)
                .selectable(true)
                .on_link_tx(|_: &mut State, url: &str, tx: &mut Tx| {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        let _ = tx.open_url(url);
                    }
                })
                .render(&prose::notes_style(t))
                .key(format!("whats-new-intro-{v}")),
        );
    }
    let mut first = intro.trim().is_empty();
    for (label, changes) in [("New", &parsed.added), ("Fixed", &parsed.fixed)] {
        if changes.is_empty() {
            continue;
        }
        body = body.child(heading(t, v, label, first)).child(list(t, v, label, changes));
        first = false;
    }
    let mut out = col().w_full().child(body);
    if parsed.internal > 0 {
        let line = words::internal_line(parsed.internal);
        let link = "All the notes on GitHub";
        let all = format!("{line} {link}");
        let start = line.len() + 1;
        out = out.child(
            rich(all, vec![Span::new(start, start + link.len()).underline(t.muted)])
                .key(format!("whats-new-internal-{v}"))
                .font(sans(12.0, 400.0, 1.45))
                .color(t.muted)
                .wrap()
                .w_full()
                .pl(24.0)
                .pb(16.0)
                .mt(-8.0)
                .link(
                    Link::new(start..start + link.len(), release.url.clone())
                        .hover(LinkPaint::new().color(t.text))
                        .focused(LinkPaint::new().ring(t.accent, 2.0)),
                )
                .on_link(|_: &mut State, url: &str, tx: &mut Tx| {
                    let _ = tx.open_url(url);
                }),
        );
    }
    out
}

/// `h4`: 11 px, uppercase, tracked 0.06em, muted; 14 above and 4 below (2 above when first).
fn heading(t: &Theme, v: &str, label: &str, first: bool) -> El {
    text(label.to_uppercase())
        .key(format!("whats-new-h-{v}-{}", label.to_lowercase()))
        .font(sans(11.0, 600.0, 1.3).tracking(0.66))
        .color(t.muted)
        .mt(if first { 2.0 } else { 14.0 })
        .mb(4.0)
}

fn list(t: &Theme, v: &str, label: &str, changes: &[ReleaseChange]) -> El {
    let mut ul = col().key(format!("whats-new-list-{v}-{}", label.to_lowercase())).w_full().gap(4.0);
    for (i, c) in changes.iter().enumerate() {
        ul = ul.child(item(t, v, label, i, c));
    }
    ul
}

/// A change: its part of Fleet as a 12 px muted label, the title, and the pull request in mono 11.5 muted that turns accent under the
/// pointer.
fn item(t: &Theme, v: &str, label: &str, at: usize, c: &ReleaseChange) -> El {
    let mut words = String::new();
    let mut spans = Vec::new();
    if let Some(scope) = &c.scope {
        spans.push(Span::new(0, scope.len()).size(12.0).color(t.muted));
        words.push_str(scope);
        words.push(' ');
    }
    words.push_str(&c.title);
    let mut link = None;
    if let Some(pr) = &c.pr {
        words.push(' ');
        let start = words.len();
        words.push_str(&format!("#{}", pr.number));
        spans.push(Span::new(start, words.len()).mono().size(11.5).color(t.muted));
        link = Some(
            Link::new(start..words.len(), pr.url.clone())
                .hover(LinkPaint::new().color(t.accent_text))
                .focused(LinkPaint::new().ring(t.accent, 2.0)),
        );
    }
    let mut p = rich(words, spans)
        .key(format!("whats-new-change-{v}-{}-{at}", label.to_lowercase()))
        .font(sans(13.5, 400.0, 1.5))
        .color(t.text)
        .wrap()
        .shrink()
        .min_w(0.0);
    if let Some(link) = link {
        p = p.link(link);
    }
    let p = p.on_link(|_: &mut State, url: &str, tx: &mut Tx| {
        if url.starts_with("https://") {
            let _ = tx.open_url(url);
        }
    });
    let bullet = el().w(4.5).h(4.5).radius(2.25).fill(t.md_marker()).mt(10.0).mr(9.0).ml(5.0);
    row().key(format!("whats-new-item-{v}-{}-{at}", label.to_lowercase())).w_full().child(bullet).child(p)
}
