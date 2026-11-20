//! Settings → Skills & tools → Tools → Custom (`tools/CustomInstallTab.vue`): the Native and MCP switch, the form for the one
//! picked, and the examples under it. What the buttons do is in `fleet/settings/tools_actions.rs`.
use super::install_picker;
use super::skills_tools::error_box;
use crate::State;
use crate::fleet::settings::install_target::Kind;
use crate::fleet::settings::tool_custom::Mode;
use crate::icons::DOWNLOAD;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, TextEdit, col, icon, row, text};

type Edit = fn(&mut State) -> &mut TextEdit;

/// `flex gap-2 rounded-card border bg-card-bg p-1`, two `flex-1 rounded-btn px-4 py-2 text-sm font-medium` buttons: the picked
/// one is filled with the accent.
fn mode_strip(s: &State, t: &Theme) -> El {
    let page = &s.settings.tools;
    let buttons = Mode::ALL.iter().map(|&mode| {
        let on = page.mode == mode;
        let b = row()
            .key(format!("tools-mode-{}", mode.id()))
            .grow(1.0)
            .w(0.0)
            .center()
            .px(14.0)
            .py(7.0)
            .radius(RADIUS_BTN)
            .fill(if on { t.accent } else { rune::Color::TRANSPARENT })
            .cursor(Cursor::Pointer)
            .role("tab")
            .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
            .child(
                text(mode.label().to_string())
                    .font(sans(rem(0.875), 500.0, 1.43))
                    .color(if on { t.primary_fg } else { t.muted })
                    .group_color(if on { t.primary_fg } else { t.text }),
            )
            .hover_group();
        if page.installing { disabled(b) } else { b.on_press(move |s: &mut State| s.set_tool_mode(mode)) }
    });
    row()
        .key("tools-mode")
        .w_full()
        .gap(7.0)
        .padding(3.5)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .role("tablist")
        .children(buttons)
}

fn label(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33).tracking(0.525)).color(t.muted)
}

/// One labelled field: `rounded-btn border px-3 py-2 text-sm`, the border turns accent while it has the focus. `rows` is the
/// textarea's height in lines, none for a one-line input (Enter submits the form there).
fn field(s: &State, t: &Theme, key: &str, words: &str, hint: &str, edit: Edit, rows: Option<usize>) -> El {
    let page = &s.settings.tools;
    let value = match key {
        "tools-native-url" => &page.native_url,
        "tools-mcp-name" => &page.mcp_name,
        "tools-mcp-command" => &page.mcp_command,
        "tools-mcp-args" => &page.mcp_args,
        _ => &page.mcp_env,
    };
    let line = sans(rem(0.875), 400.0, 1.43);
    let mut input = text_input(value, line)
        .key(key.to_string())
        .label(words.to_string())
        .placeholder(hint.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_BTN)
        .border(1.0, if value.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .on_edit(edit);
    input = match rows {
        None => input.single_line().on_submit(|s: &mut State, _| s.install_custom_tool()),
        // A textarea of `rows="4"`: four lines, then it scrolls.
        Some(n) => input.h(rem(0.875) * 1.43 * n as f32 + rem(1.0) + 2.0),
    };
    if page.installing {
        input = input.disabled(true);
    }
    col().w_full().gap(3.5).child(label(t, words)).child(input)
}

fn heading(t: &Theme, title: &str, line: &str) -> El {
    col()
        .child(text(title.to_string()).font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(row().mt(3.5).child(text(line.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)))
}

fn submit(s: &State, t: &Theme) -> El {
    let page = &s.settings.tools;
    let words = if page.mode == Mode::Native { "Install tool" } else { "Install MCP server" };
    let lead = if page.installing { spinner(t.primary_fg) } else { icon(&DOWNLOAD, 16.0).color(t.primary_fg) };
    let b = button_lead(t, Variant::Default, Size::Default, Some(lead), if page.installing { "Installing\u{2026}" } else { words })
        .key("tools-custom-submit");
    row().child(if page.installing { disabled(b) } else { b.on_press(|s: &mut State| s.install_custom_tool()) })
}

/// The picked form's card: the heading, the picker, the fields, the error, the button. MCP's picker says `opencode.json`.
fn form(s: &State, t: &Theme) -> El {
    let page = &s.settings.tools;
    let mut f = col()
        .key("tools-custom-form")
        .w_full()
        .gap(10.5)
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg);
    f = match page.mode {
        Mode::Native => f
            .child(heading(t, "Install from URL or path", "Install a native tool from a GitHub URL or local file path."))
            .child(install_picker::picker(s, t, "tools-target", Kind::Tools))
            .child(field(
                s,
                t,
                "tools-native-url",
                "Tool URL or path",
                "https://github.com/user/repo/blob/main/tool.ts or /path/to/tool.ts",
                |s| &mut s.settings.tools.native_url,
                None,
            )),
        Mode::Mcp => f
            .child(heading(t, "Configure MCP server", "Install an MCP server with custom command, arguments, and environment variables."))
            .child(install_picker::picker(s, t, "tools-target", Kind::Config))
            .child(field(s, t, "tools-mcp-name", "Name", "my-mcp-server", |s| &mut s.settings.tools.mcp_name, None))
            .child(field(s, t, "tools-mcp-command", "Command", "node", |s| &mut s.settings.tools.mcp_command, None))
            .child(field(
                s,
                t,
                "tools-mcp-args",
                "Arguments (one per line)",
                "/path/to/server.js\n--port\n3000",
                |s| &mut s.settings.tools.mcp_args,
                Some(4),
            ))
            .child(field(
                s,
                t,
                "tools-mcp-env",
                "Environment variables (KEY=VALUE per line)",
                "API_KEY=your-key\nDEBUG=true",
                |s| &mut s.settings.tools.mcp_env,
                Some(4),
            )),
    };
    if let Some(e) = &page.custom_error {
        f = f.child(error_box(t, "tools-custom-error", e));
    }
    f.child(submit(s, t))
}

/// The Examples card: Native's three addresses, or MCP's one server.
fn examples(s: &State, t: &Theme) -> El {
    let strong = |words: &str| text(words.to_string()).font(sans(rem(0.75), 600.0, 1.33)).color(t.text);
    let line = |words: &str| row().child(text(words.to_string()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted));
    let pair = |name: &str, value: &str| {
        row()
            .gap(3.5)
            .child(text(name.to_string()).font(sans(rem(0.75), 600.0, 1.33)).color(t.muted))
            .child(text(value.to_string()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted).wrap_anywhere())
    };
    let body = match s.settings.tools.mode {
        Mode::Native => col().key("tools-custom-examples-native").mt(7.0).gap(3.5).child(strong("Native tools:")).child(
            col()
                .gap(3.5)
                .child(line("https://github.com/username/repo/blob/main/tools/my-tool.ts"))
                .child(line("https://github.com/username/my-tool"))
                .child(line("/Users/username/tools/my-tool.ts")),
        ),
        Mode::Mcp => col().key("tools-custom-examples-mcp").mt(7.0).child(strong("MCP server example:")).child(
            col()
                .mt(3.5)
                .gap(3.5)
                .child(pair("Name:", "filesystem"))
                .child(pair("Command:", "npx"))
                .child(pair("Args:", "-y @modelcontextprotocol/server-filesystem /path/to/allowed/files")),
        ),
    };
    col()
        .key("tools-custom-examples")
        .w_full()
        .padding(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(text("Examples").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(body)
}

/// The Custom tab (`space-y-4`: 14 between the strip, the form and the examples).
pub fn custom_tab(s: &State, t: &Theme) -> El {
    col().key("tools-custom").w_full().gap(14.0).child(mode_strip(s, t)).child(form(s, t)).child(examples(s, t))
}
