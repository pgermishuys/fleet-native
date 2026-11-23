//! The Board's right panel (`BoardRightPanel.vue`, `RightPanelTabs.vue`, `AppShell.vue`'s board column): the tabs row with its collapse
//! button, the body, and the gutter it shares with Sessions' panel (one width, one drag). Folding hides the panel and leaves the
//! gutter, with no collapsed rail (FB8); Cmd/Ctrl+Shift+B or the palette brings it back.
use super::{activity, summary};
use crate::State;
use crate::fleet::board::panel::Tab;
use crate::fleet::right_panel as rp;
use crate::icons::PANEL_RIGHT_CLOSE;
use crate::theme::{Theme, sans};
use crate::view::{parts::tool_button, right_rail};
use rune::{Cursor, El, ScrollSpec, col, el, row, text};

/// `.right-tab`: flex 1, padding 8 0, 10 px 500 muted; the open one `--text` over a 2 px accent line.
fn tab(s: &State, t: &Theme, tab: Tab) -> El {
    let open = s.board.right.tab == tab;
    let ink = if open { t.text } else { t.muted };
    el().key(tab.key())
        .label(tab.label())
        .grow(1.0)
        .min_w(0.0)
        .center()
        .py(8.0)
        .border_sides([0.0, 0.0, 2.0, 0.0], if open { t.accent } else { rune::Color::TRANSPARENT })
        .border_layout()
        .hover_group()
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.board_pick_tab(tab))
        .child(text(tab.label()).font(sans(10.0, 500.0, 1.5)).color(ink).group_color(t.text))
}

/// `RightPanelTabs`: the tablist, then the collapse button (34 wide, `border-left`).
fn tabs(s: &State, t: &Theme) -> El {
    row()
        .key("board-right-tabs")
        .w_full()
        .items_stretch()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(row().grow(1.0).min_w(0.0).items_stretch().children(Tab::ALL.iter().map(|&x| tab(s, t, x))))
        .child(
            el().w(34.0).center().border_sides([0.0, 0.0, 0.0, 1.0], t.border).child(
                tool_button(t, "board-right-collapse", &PANEL_RIGHT_CLOSE, 14.0, "Collapse right panel")
                    .on_press(|s: &mut State| s.toggle_board_right()),
            ),
        )
}

/// The open panel: border-left, the tabs, then the body (padding 14 14 20, scrolling).
pub fn panel(s: &State, t: &Theme) -> El {
    let body = match s.board.right.tab {
        Tab::Summary => summary::view(s, t),
        Tab::Activity => activity::view(s, t),
    };
    col()
        .key("board-right-panel")
        .label("Right panel")
        .w(s.right_shown_w())
        .min_w(rp::MIN_W)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .child(tabs(s, t))
        .child(
            el().key("board-right-body")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(col().key("board-right-content").w_full().px(14.0).pt(14.0).pb(20.0).child(body)),
        )
}

/// What goes after the page inside the sheet: the gutter while the panel could be a column, the panel while it is open.
pub fn columns(s: &State, t: &Theme) -> Vec<El> {
    if !rp::fits(s.sheet_w) {
        return Vec::new();
    }
    let mut out = vec![right_rail::gutter(t)];
    if !s.sidebar.right_collapsed(None) {
        out.push(panel(s, t));
    }
    out
}
