//! Settings → Skills & tools → Skills → Catalog and Custom (`skills/CatalogTab.vue`, `skills/CustomInstallTab.vue`): the cards
//! the catalog offers with Install, and the form that installs from a GitHub address or a folder.
use super::install_picker;
use super::skills_tools::{empty_state, error_box, loading};
use crate::State;
use crate::fleet::settings::install_target::Kind;
use crate::fleet::settings::skills::SkillsPage;
use crate::fleet::settings::skills_catalog::{CatalogEntry, is_installed};
use crate::icons::{BRAIN, DOWNLOAD};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::controls::{BadgeVariant, badge};
use crate::ui::rem;
use rune::{El, col, icon, row, text, text_input};

/// The Install button, or Installed (outline, disabled) where the picker points, or Installing… while it runs.
fn action(s: &State, t: &Theme, entry: &CatalogEntry) -> El {
    let page = &s.settings.skills;
    if is_installed(&page.skills, &entry.name, &page.target) {
        return disabled(button(t, Variant::Outline, Size::Sm, None, "Installed").key(format!("catalog-installed-{}", entry.name)));
    }
    let busy = page.installing_name.as_deref() == Some(entry.name.as_str());
    let lead = if busy { spinner(t.primary_fg) } else { icon(&DOWNLOAD, 16.0).color(t.primary_fg) };
    let b = button_lead(t, Variant::Default, Size::Sm, Some(lead), if busy { "Installing\u{2026}" } else { "Install" })
        .key(format!("catalog-install-{}", entry.name));
    if busy {
        disabled(b)
    } else {
        let name = entry.name.clone();
        b.on_press(move |s: &mut State| s.install_catalog_skill(&name))
    }
}

/// `rounded-card border bg-main-bg p-4` with `gap-3` inside.
fn card(s: &State, t: &Theme, entry: &CatalogEntry) -> El {
    let mut title = col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .child(text(entry.title().to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text).truncate().shrink().min_w(0.0));
    if let Some(author) = entry.author.as_ref().filter(|a| !a.is_empty()) {
        title = title.child(row().mt(1.75).child(text(format!("by {author}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)));
    }
    let mut c = col()
        .key(format!("catalog-card-{}", entry.name))
        .w_full()
        .gap(10.5)
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(row().gap(7.0).items_start().child(icon(&BRAIN, 16.0).color(t.muted).mt(1.75)).child(title));
    if let Some(about) = entry.description.as_ref().filter(|d| !d.is_empty()) {
        c = c.child(text(about.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).max_lines(3).w_full());
    }
    if !entry.tags.is_empty() {
        c = c.child(row().gap(3.5).wrap().children(entry.tags.iter().map(|tag| badge(t, BadgeVariant::Outline, tag))));
    }
    let mut left = row().gap(7.0).center_y().child(badge(t, BadgeVariant::Secondary, entry.source_label()));
    if let Some(v) = entry.version.as_ref().filter(|v| !v.is_empty()) {
        left = left.child(text(format!("v{v}")).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    }
    c.child(row().w_full().justify_between().center_y().gap(7.0).pt(7.0).child(left).child(action(s, t, entry)))
}

/// `grid gap-3 sm:grid-cols-2`: rows of two equal columns; a last card on its own keeps its half.
fn grid(s: &State, t: &Theme, page: &SkillsPage) -> El {
    grid_of("skills-catalog-grid", page.catalog.iter().map(|entry| card(s, t, entry)).collect())
}

/// The grid itself, for a catalog of any kind: `cards` two to a row.
pub(super) fn grid_of(key: &str, cards: Vec<El>) -> El {
    let mut g = col().key(key.to_string()).w_full().gap(10.5);
    let mut cards = cards.into_iter();
    while let Some(first) = cards.next() {
        let second = cards.next();
        let mut r = row().w_full().gap(10.5);
        r = r.child(col().w(0.0).grow(1.0).min_w(0.0).child(first.grow(1.0)));
        if let Some(entry) = second {
            r = r.child(col().w(0.0).grow(1.0).min_w(0.0).child(entry.grow(1.0)));
        } else {
            r = r.child(col().w(0.0).grow(1.0));
        }
        g = g.child(r);
    }
    g
}

/// The Catalog tab: the picker, then loading, the error alone, the empty state or the grid; the install's error (else the
/// load's, when there is a catalog) under it.
pub fn catalog_tab(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let has = !page.catalog.is_empty();
    let mut body = col().key("skills-catalog").w_full().gap(14.0).child(install_picker::picker(s, t, "skills-target", Kind::Skills));
    if page.catalog_loading {
        body = body.child(loading(t, "skills-catalog-loading", "Loading catalog\u{2026}"));
    } else if let (Some(e), false) = (&page.catalog_error, has) {
        body = body.child(error_box(t, "skills-catalog-error", e));
    } else if !has {
        body = body.child(empty_state(
            t,
            "skills-catalog-empty",
            &BRAIN,
            "No skills in catalog",
            "The skill catalog is empty or unavailable.",
        ));
    } else {
        body = body.child(grid(s, t, page));
    }
    if let Some(e) = &page.install_error {
        body = body.child(error_box(t, "skills-install-error", e));
    } else if let (Some(e), true, false) = (&page.catalog_error, has, page.catalog_loading) {
        body = body.child(error_box(t, "skills-catalog-error", e));
    }
    body
}

fn field(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let input = text_input(&page.custom, sans(rem(0.875), 400.0, 1.43))
        .key("skills-custom-input")
        .label("Skill URL or path")
        .single_line()
        .placeholder("https://github.com/user/skill or /path/to/skill".to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_BTN)
        .border(1.0, if page.custom.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .on_edit(|s: &mut State| &mut s.settings.skills.custom)
        .on_submit(|s: &mut State, _| s.install_custom_skill());
    let input = if page.installing { input.disabled(true) } else { input };
    col().w_full().gap(3.5).child(text("SKILL URL OR PATH").font(sans(rem(0.75), 500.0, 1.33).tracking(0.525)).color(t.muted)).child(input)
}

fn form(s: &State, t: &Theme) -> El {
    let page = &s.settings.skills;
    let lead = if page.installing { spinner(t.primary_fg) } else { icon(&DOWNLOAD, 16.0).color(t.primary_fg) };
    let submit =
        button_lead(t, Variant::Default, Size::Default, Some(lead), if page.installing { "Installing\u{2026}" } else { "Install skill" })
            .key("skills-custom-submit");
    let submit = if page.installing { disabled(submit) } else { submit.on_press(|s: &mut State| s.install_custom_skill()) };
    let mut f =
        col()
            .key("skills-custom-form")
            .w_full()
            .gap(10.5)
            .padding(14.0)
            .radius(RADIUS_CARD)
            .border(1.0, t.border)
            .border_layout()
            .fill(t.main_bg)
            .child(
                col().child(text("Install from URL or path").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)).child(row().mt(3.5).child(
                    text("Install a skill from a GitHub URL or local file path.").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted),
                )),
            )
            .child(install_picker::picker(s, t, "skills-target", Kind::Skills))
            .child(field(s, t));
    if let Some(e) = &page.custom_error {
        f = f.child(error_box(t, "skills-custom-error", e));
    }
    f.child(row().child(submit))
}

fn examples(t: &Theme) -> El {
    let line = |words: &str| row().child(text(words.to_string()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted));
    col()
        .key("skills-custom-examples")
        .w_full()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(text("Examples").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(
            col()
                .mt(7.0)
                .gap(3.5)
                .child(line("https://github.com/username/skill-name"))
                .child(line("https://github.com/username/repo/tree/main/skills/my-skill"))
                .child(line("/Users/username/skills/custom-skill")),
        )
}

/// The Custom tab: the form, and the examples under it.
pub fn custom_tab(s: &State, t: &Theme) -> El {
    col().key("skills-custom").w_full().gap(14.0).child(form(s, t)).child(examples(t))
}
