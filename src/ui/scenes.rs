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
