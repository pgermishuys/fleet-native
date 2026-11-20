//! The kit's scenes (`--scene kit-menu`, and `kit-menu-light` for the light theme): one per
//! control state worth looking at.
use super::{Open, dialog::SheetSide, toast::Toast};
use crate::State;
use rune::TextEdit;

pub fn set(s: &mut State, name: &str, dark: bool) {
    s.dark = Some(dark);
    let gallery = |s: &mut State, open: Open| {
        s.gallery = true;
        s.ui.open = Some(open);
    };
    match name {
        "kit" => s.gallery = true,
        "kit-palette" => s.ui.open = Some(Open::Palette),
        "kit-palette-level" => {
            s.ui.open = Some(Open::Palette);
            s.ui.palette.level = Some("nav-go-to-session".into());
        }
        "kit-palette-search" => {
            s.ui.open = Some(Open::Palette);
            s.ui.palette.query = TextEdit::new("th");
        }
        "kit-agent-menu" => s.ui.open = Some(Open::Menu("agent")),
        "kit-more-menu" => s.ui.open = Some(Open::Menu("more")),
        "kit-context-menu" => {
            s.ui.point = (360.0, 230.0);
            s.ui.open = Some(Open::Context(0, 2));
        }
        // The text menu over a field with a selection and over a link (`ui::text_menu`), at fixed points in the composer and the
        // conversation.
        "text-menu-field" => {
            s.draft = TextEdit::new("Ship the paging fix");
            let selected = "paging".to_owned();
            let params = super::text_menu::Params {
                editable: true,
                can_cut: true,
                can_copy: true,
                can_paste: true,
                selection: selected,
                link: None,
            };
            s.open_text_menu(params, None, (470.0, 650.0));
        }
        "text-menu-link" => {
            let params = super::text_menu::Params { link: Some("https://example.test/docs/rounding".into()), ..Default::default() };
            s.open_text_menu(params, None, (620.0, 300.0));
        }
        "kit-toast" => {
            s.ui.toast = Some(Toast {
                kind: Default::default(),
                message: "Archived “Tidy the release notes”.".into(),
                action: Some("Undo".into()),
                until: 1.0e9,
                total: 1.0e9,
            })
        }
        "kit-menu" => gallery(s, Open::Menu("gallery")),
        "kit-select" => gallery(s, Open::Select("g")),
        "kit-popover" => gallery(s, Open::Popover("g")),
        "kit-directory" => {
            s.ui.dir.at = vec!["code"];
            gallery(s, Open::Directory);
        }
        "kit-dialog" => gallery(s, Open::Dialog),
        "kit-alert" => gallery(s, Open::Alert),
        "kit-sheet" => gallery(s, Open::Sheet(SheetSide::Right)),
        _ => {}
    }
}
