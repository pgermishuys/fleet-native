# Component 1: Kit — parity checklist

Ported from weave-fleet `main` at `592ade13` (14 November 2026), `client/src/components/ui/`,
`components/CommandPalette.vue`, `composables/use-commands.ts`, `lib/keybinding-types.ts`.
Fleet's server package is v0.72.0; nothing in the kit differs between the two that we found.

Built in `src/ui/`, one file per control. Tokens come from `src/theme.rs` only. Underneath, the
controls use `rune-kit` through **one Fleet-derived `Kit`** (`src/ui/kit.rs`: `Kit::neutral` with
Fleet's colours, radii, type, shadows and glyph weights overwritten), so Menu, Select, Combobox,
Dialog, Toast, Tooltip and Popover keep Rune's keyboard, focus and flip behaviour and look like
Fleet's. Where rune-kit can't match, the control is ours and the row says why.

Legend: `[ ]` open · `[x]` done · `[-]` out of reach, with the reason.

## Tokens (`src/theme.rs`)

CSS variable beside each field.

- [ ] `--primary`, `--primary-foreground`, `--destructive`, `--destructive-foreground`
- [ ] `--accent-dim`, `--complete`, `--queued`
- [ ] `--menu-shadow` (both themes), `--radius-card` 10, `--radius-btn` 8, `--radius-panel` 12
- [ ] Derived names `ui/` uses (`main.css`, `@theme inline`): `--color-secondary` (text 8%),
      `--color-input` (text 18%), `--color-ring` (accent), `--color-accent-foreground`,
      `--color-popover` (= card-bg), `--color-background` (= panel-bg)
- [ ] `--transition` 150 ms ease-out (used as the control spring)

## Controls

| Fleet file | Native | Sizes, states, keys |
|---|---|---|
| `button/` (`buttonVariants`) | `ui/button.rs` | [ ] variants default, destructive, outline, secondary, ghost, link, toolbar-icon, toolbar-icon-danger, filter · [ ] sizes default 36, sm 32, lg 40, icon 36, icon-sm 32, icon-lg 40, toolbar 28, toolbar-lg 32 · [ ] hover, pressed, disabled (50%), focus ring (3 px accent 50%) · [ ] Enter/Space press |
| `badge/` | `ui/badge.rs` | [ ] default, secondary, destructive, outline, status (square, as Fleet's `rounded-none`) |
| `input/`, `textarea/` | `ui/input.rs` | [ ] 36 px field, border `--input`, placeholder, focus ring, invalid ring, disabled · [ ] textarea min 64 px, grows |
| `switch/` | `ui/switch.rs` | [ ] 32 × 18.4 track, 16 px thumb, checked/unchecked/disabled · Space and Enter toggle |
| `separator/` | `ui/separator.rs` | [ ] horizontal, vertical |
| `tabs/` | `ui/tabs.rs` | [ ] default (segmented) and underline variants · [ ] Left/Right move and activate, Home/End |
| `progress/` | `ui/progress.rs` | [ ] 8 px track, primary 20%, fill |
| `card/` | `ui/card.rs` | [ ] card, header, title, description, content, footer, action; radius 10, py 24, px 24 |
| `avatar/` | `ui/avatar.rs` | [ ] 32 px circle, image or fallback letter |
| `scroll-area/` | `ui/scroll.rs` | [ ] Rune scroll with Fleet's thin bar (`bg-border` thumb) |
| `collapsible/` | `ui/collapsible.rs` | [ ] trigger and content, open/closed; Enter/Space |
| `tooltip/` | `ui/tooltip.rs` | [ ] 4 px offset, `px-3 py-1.5 text-xs`, foreground fill with background ink, arrow · delay as reka's default (700 ms, 300 ms after one was open) |
| `menu-classes.ts`, `dropdown-menu/` | `ui/menu.rs` | [ ] panel: card-bg, border, radius 10, padding 5, `--menu-shadow`, min 128 · [ ] row: min 30, radius 6, `px-2 py-[5px]`, 13 px, highlight `text 7%`, icon 14 px muted → text · [ ] destructive row (red, red 12% highlight) · [ ] label (11 px, semibold, uppercase, 0.05em), separator (`mx-0.5 my-1`), shortcut chip · [ ] checkbox and radio items with indicator, inset (pl 32) · [ ] submenu (open stays tinted) · [ ] disabled 45% · [ ] open/close 120 ms fade + scale .97 · [ ] Up/Down/Home/End, typeahead, Enter/Space, Right/Left for submenus, Escape, Tab closes · [ ] focus returns to the trigger |
| `context-menu/` | `ui/menu.rs` | [ ] same panel and rows, opens at the pointer (Rune `context_menu`; Rune draws it on every system, no native menu) · [ ] Shift+F10 / the menu key |
| `popover/` | `ui/popover.rs` | [ ] card-bg, border, 288 wide, padding 16, `shadow-md`, 4 px offset, flips · [ ] Escape and outside click close |
| `select/` | `ui/select.rs` | [ ] trigger 36 / 32 (sm), chevron 16 at 50%, border `--input` · [ ] content min trigger width, padding 4, rows `py-1.5 pl-2 pr-8`, check at the right, highlight accent fill with accent-foreground · [ ] Up/Down, Home/End, typeahead, Enter, Escape |
| `command/` | `ui/command.rs` | [ ] search row (icon, input, bottom border), groups with 12 px headings, items `px-2 py-1.5`, accent highlight, empty text, shortcut chips · [ ] Up/Down wrap, Enter, Escape, typeahead is the filter |
| `dialog/`, `responsive-dialog/` | `ui/dialog.rs` | [ ] overlay black 80%, content `max-w-lg` (512), padding 24, gap 16, border, shadow-lg · [ ] title 18 semibold, description 14 muted, footer right-aligned, close button (top right, 70% opacity) · [ ] Escape, click outside, focus trapped · [-] the bottom-sheet form (phone layout is out of scope) |
| `alert-dialog/` | `ui/dialog.rs` | [ ] as dialog with no close button, no click-outside; Cancel (outline) and Action (default) · Escape cancels |
| `sheet/` | `ui/sheet.rs` | [ ] right, left, top, bottom; max 384 wide for sides; slide 300 ms out / 500 ms in |
| `DirectoryPickerPopover.vue` | `ui/directory_picker.rs` | [ ] the popover (512 wide, breadcrumb bar, roots header with refresh, list rows with folder / repo icons, search, "Use this directory" in navigate mode) over a model with entries · [-] listing a real server's folders: that is component 7's New session |
| Toasts (`vue-sonner` in `App.vue`) | `ui/toast.rs` | [ ] one toast, a stack; closable; action; auto-dismiss · [ ] bottom-right, as Fleet's |

## Command palette

From `CommandPalette.vue`, opened by Ctrl K (⌘K on macOS) and the status bar's `Ctrl K` hint.

- [ ] Dialog: no close button; input placeholder "Type a command or search…" (sub-level: "<parent>…")
- [ ] List max 300 px; groups in the order Session, Navigation, View, Fleet; "No commands found."
- [ ] Each row: icon, label, description (muted 12 px), `›` for a sub-level, else the shortcut chip
- [ ] Fleet's filter (cmdk-style: label and keywords, contiguous matches first); ranking is ours
- [ ] Sub-levels (Go to session…): a Back row; Backspace on an empty input goes back
- [ ] Footer: `↑↓ Navigate`, `↵ Select`, `⌫ Back` (in a sub-level), `Esc Close`, `Ctrl+K toggle`
- [ ] Disabled commands are listed, dimmed, and don't run
- [ ] Ctrl K again closes it; closing resets the query and the level
- [ ] Commands (id → what the native app does):
  - runs now: `nav-fleet` Go to Sessions, `nav-settings` Go to Settings, `nav-analytics`,
    `nav-go-to-session` (sub-level of the sessions), `nav-next-session`, `nav-prev-session`,
    `interrupt-session`, `copy-session-id`, `toggle-sidebar`, `toggle-right-panel`,
    `cycle-theme`, `toggle-dark-light`, `focus-prompt`, `clear-conversation`, `scroll-to-top`,
    `scroll-to-bottom`, `new-session` (focuses the composer on the blank session, as far as the
    shell has one)
  - listed, marked "waiting on component N": `nav-board` (23), `add-machine` (17),
    `refresh-sessions` (2), `go-to-file` and `find-in-files` (11), `fork-session` (8),
    `new-session-in-folder` (7), `export-conversation` and `export-raw-messages` (3),
    `toggle-terminal` (13), `toggle-diff-view` (3), `toggle-activity-filter` (6),
    `toggle-fullscreen` (24), `zoom-in` and `zoom-out` (9), `open-marketplace-panel` (16),
    `report-problem` (9), `start-without-mods` (22)
- [ ] Global shortcuts the palette shows run from the window too, where the command runs

## Where the shell uses the kit

- [ ] Rail: tooltips on every entry (Fleet's right-side tooltip)
- [ ] Session header and the status bar: buttons (toolbar-icon), the Ctrl K chip opens the palette
- [ ] Composer: the agent and model chips open a menu; the send button is a Button
- [ ] Session list: filter field is the kit's input; a row's right-click opens a context menu

## Pictures

Each control open, native beside Fleet's own client (a scratch Fleet's page, same size), both
themes: `shots/parity/01-kit/`.

## Out of reach, with reasons

- **Animation curves**: Fleet's `tw-animate-css` zoom/slide; Rune springs approximate them
  (checked by eye, not frame by frame).
- **Native right-click menus**: Rune has them on macOS and Windows only; Fleet's own menus are
  drawn, so drawing is the parity.
