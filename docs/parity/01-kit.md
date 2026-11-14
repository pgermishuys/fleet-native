# Component 1: Kit — parity checklist

Ported from weave-fleet `main` at `592ade13` (14 November 2026), `client/src/components/ui/`,
`components/CommandPalette.vue`, `composables/use-commands.ts`, `lib/keybinding-types.ts`.
Fleet's server package is v0.72.0; nothing in the kit differs between the two that we found.

Built in `src/ui/`, one file per control group. Tokens come from `src/theme.rs` only. `rune-kit`'s
Menu, Select, Dialog and Popover hardcode their sizes (a 6px radius, 28px rows), so they can't be
styled as Fleet's through a `Kit`: the controls are built on Rune's own primitives (anchoring,
modal, focus groups, `HelpLook`) at Fleet's sizes. Overlays are mounted once at the root from one
`Ui::open` value (`docs/RUNE.md`, "What the kit taught us").

Legend: `[x]` open · `[x]` done · `[-]` out of reach, with the reason.

## Tokens (`src/theme.rs`)

CSS variable beside each field.

- [x] `--primary`, `--primary-foreground`, `--destructive`, `--destructive-foreground`
- [x] `--accent-dim`, `--complete`, `--queued`
- [x] `--menu-shadow` (both themes), `--radius-card` 10, `--radius-btn` 8, `--radius-panel` 12
- [x] Derived names `ui/` uses (`main.css`, `@theme inline`): `--color-secondary` (text 8%),
      `--color-input` (text 18%), `--color-ring` (accent), `--color-accent-foreground`,
      `--color-popover` (= card-bg), `--color-background` (= panel-bg)
- [x] `--transition` 150 ms ease-out (used as the control spring)

## Controls

| Fleet file | Native | Sizes, states, keys |
|---|---|---|
| `button/` (`buttonVariants`) | `ui/button.rs` | [x] variants default, destructive, outline, secondary, ghost, link, toolbar-icon, toolbar-icon-danger, filter · [ ] ghost's border on hover and link's underline (Rune has neither, `docs/RUNE.md`) · [x] sizes (`h-9` etc. at 14px rem) default 31.5, sm 28, lg 35, icon 31.5, icon-sm 28, icon-lg 35, toolbar 24.5, toolbar-lg 28 · [x] hover, pressed, disabled (50%), focus ring (3 px accent 50%) · [x] Enter/Space press |
| `badge/` | `ui/badge.rs` | [x] default, secondary, destructive, outline, status (square, as Fleet's `rounded-none`) |
| `input/`, `textarea/` | `ui/input.rs` | [x] 31.5 px field, border `--input`, placeholder, focus ring, invalid ring, disabled · [x] textarea min 64 px, grows |
| `switch/` | `ui/switch.rs` | [x] 28 × 16.1 track, 14 px thumb, checked/unchecked/disabled · Space and Enter toggle |
| `separator/` | `ui/separator.rs` | [x] horizontal, vertical |
| `tabs/` | `ui/tabs.rs` | [x] default (segmented) and underline variants · [x] Left/Right move and activate, Home/End |
| `progress/` | `ui/progress.rs` | [x] 7 px track, primary 20%, fill |
| `card/` | `ui/card.rs` | [x] card, header, title, description, content, footer, action; radius 10, py 24, px 24 |
| `avatar/` | `ui/avatar.rs` | [x] 28 px circle, image or fallback letter |
| `scroll-area/` | `ui/scroll.rs` | [ ] Rune's `.scroll()` is used as it is (palette, select, directory lists); Fleet's thin `bg-border` bar is not restyled |
| `collapsible/` | `ui/controls.rs` | [x] the trigger row with its chevron; the caller shows the content (not on screen anywhere yet) |
| `tooltip/` | `ui/tooltip.rs` | [x] 4 px offset, `px-3 py-1.5 text-xs`, foreground fill with background ink (`Cx::help_look`; no arrow, Rune's tooltip has none) · delay as reka's default (700 ms, 300 ms after one was open) |
| `menu-classes.ts`, `dropdown-menu/` | `ui/menu.rs` | [x] panel: card-bg, border, radius 10, padding 5, `--menu-shadow`, min 128 · [x] row: min 30, radius 6, `px-2 py-[5px]`, 13 px, highlight `text 7%`, icon 14 px muted → text · [x] destructive row (red, red 12% highlight) · [x] label (11 px, semibold, uppercase, 0.05em), separator (`mx-0.5 my-1`), shortcut chip · [x] checkbox and radio items with indicator, inset (pl 32) · [ ] submenu (the row and chevron draw; opening one isn't built: no menu in the shell uses it yet) · [x] disabled 45% · [x] open: fade + scale .97 from the trigger side (a spring, not 120 ms exactly) · [x] Up/Down, Enter and Escape (tested); Home/End and typeahead are Rune's focus group's, not tested here; Right/Left wait on submenus · [x] focus returns to the trigger |
| `context-menu/` | `ui/menu.rs` | [x] same panel and rows, opens at the pointer (Rune `context_menu`; Rune draws it on every system, no native menu) · [x] Shift+F10 / the menu key |
| `popover/` | `ui/popover.rs` | [x] card-bg, border, 288 wide, padding 16, `shadow-md`, 4 px offset, flips · [x] Escape and outside click close |
| `select/` | `ui/select.rs` | [x] trigger 31.5 / 28 (sm), chevron 16 at 50%, border `--input` · [x] content min trigger width, padding 4, rows `py-1.5 pl-2 pr-8`, check at the right, highlight accent fill with accent-foreground · [x] Up/Down, Home/End, typeahead, Enter, Escape |
| `command/` | `ui/command.rs` | [x] search row (icon, input, bottom border), groups with 12 px headings, items `px-2 py-1.5`, accent highlight, empty text, shortcut chips · [x] Up/Down wrap, Enter, Escape, typeahead is the filter |
| `dialog/`, `responsive-dialog/` | `ui/dialog.rs` | [x] overlay black 80%, content `max-w-lg` (448), padding 21, gap 14, border, shadow-lg · [x] title 18 semibold, description 14 muted, footer right-aligned, close button (top right, 70% opacity) · [x] Escape, click outside, focus trapped · [-] the bottom-sheet form (phone layout is out of scope) |
| `alert-dialog/` | `ui/dialog.rs` | [x] as dialog with no close button, no click-outside; Cancel (outline) and Action (default) · Escape cancels |
| `sheet/` | `ui/dialog.rs` | [x] right, left, top, bottom drawn (the right one is in the gallery) · [ ] the 300/500 ms slide: it scales in from its edge on a spring |
| `DirectoryPickerPopover.vue` | `ui/directory_picker.rs` | [x] the popover (448 wide, breadcrumb bar, roots header with refresh, list rows with folder / repo icons, search, "Use this directory" in navigate mode) over a model with entries · [-] listing a real server's folders: that is component 7's New session |
| Toast (`sessions/ArchiveUndoToast.vue`; the kit has no sonner) | `ui/toast.rs` | [x] the inverted bar at the bottom centre, 420 wide, message, pill action, 2 px drain bar, auto-dismiss · [-] the error variant with a dismiss X (no error to show yet) |

## Command palette

From `CommandPalette.vue`, opened by Ctrl K (⌘K on macOS) and the status bar's `Ctrl K` hint.

- [x] Dialog: no close button; input placeholder "Type a command or search…" (sub-level: "<parent>…")
- [x] List max 300 px; groups in the order Session, Navigation, View, Fleet; "No commands found."
- [x] Each row: icon, label, description (muted 12 px), `›` for a sub-level, else the shortcut chip
- [x] Filter on label, keywords and description; ranking is ours (cmdk's scorer differs); unfiltered groups are alphabetical, as Fleet's are
- [x] Sub-levels (Go to session…): a Back row; Backspace on an empty input goes back
- [x] Footer: `↑↓ Navigate`, `Enter Select`, `Backspace Back` (in a sub-level; ↵ and ⌫ draw as boxes in our fonts), `Esc Close`, `Ctrl+K toggle`
- [x] Disabled commands are listed, dimmed, and don't run
- [x] Ctrl K again closes it; closing resets the query and the level
- [x] Commands (id → what the native app does):
  - runs now: `nav-fleet` Go to Sessions, `nav-settings` Go to Settings, `nav-analytics`,
    `nav-go-to-session` (sub-level of the sessions), `nav-next-session`, `nav-prev-session`,
    `interrupt-session`, `toggle-sidebar`, `toggle-right-panel`,
    `cycle-theme`, `toggle-dark-light`, `focus-prompt`, `clear-conversation`, `scroll-to-top`,
    `scroll-to-bottom`
  - listed, marked "waiting on component N": `nav-board` (23), `add-machine` (17),
    `new-session` (7), `copy-session-id` (9: no clipboard write from a handler at this pin), `refresh-sessions` (2), `go-to-file` and `find-in-files` (11), `fork-session` (8),
    `new-session-in-folder` (7), `export-conversation` and `export-raw-messages` (3),
    `toggle-terminal` (13), `toggle-diff-view` (3), `toggle-activity-filter` (6),
    `toggle-fullscreen` (24), `zoom-in` and `zoom-out` (9), `open-marketplace-panel` (16),
    `report-problem` (9), `start-without-mods` (22)
- [x] Ctrl K (⌘K), Ctrl B and Ctrl Shift B run from the window; the other shortcuts wait with their commands

## Where the shell uses the kit

- [x] Rail: tooltips on every entry, in Fleet's tooltip look
- [x] Session header and the status bar: buttons (toolbar-icon), More actions opens Fleet's menu and the Ctrl K chip opens the palette · [ ] the header's other buttons are still the shell's own
- [x] Composer: the agent and model chips open a menu; [ ] the send button is still the shell's own element
- [x] Session list: filter field is the kit's input; a row's right-click (and the menu key) opens a context menu · [ ] the filter field is still the shell's own

## Pictures

In `shots/parity/01-kit/`: `fleet-*` is Fleet's own client (a scratch v0.72.0), `native-*` the native app, 1280 × 760 at 2×, both themes. Fleet's palette, its Go to session level and the session's More actions menu are paired with ours. The other overlays (menu, select, popover, directory, dialog, alert, sheet, toast, the agent and context menus) are native-only, in the kit gallery: Fleet shows them only inside other pages (components 5 to 9), so a same-state pair comes with those components.

Each control open, native beside Fleet's own client (a scratch Fleet's page, same size), both
themes: `shots/parity/01-kit/`.

## Out of reach, with reasons

- **Animation curves**: Fleet's `tw-animate-css` zoom/slide; Rune springs approximate them
  (checked by eye, not frame by frame).
- **Native right-click menus**: Rune has them on macOS and Windows only; Fleet's own menus are
  drawn, so drawing is the parity.
