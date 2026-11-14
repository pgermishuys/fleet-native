# The port plan

How Fleet's desktop app becomes a native one, in pieces that separate sessions can build without
getting in each other's way.

Tracks weave-fleet **v0.72.0**. The Vue client changes every day, so "the same as Fleet" means the
same as that version until a catch-up pass moves this line.

## The question this repository answers

Is a native Fleet enough better than the Electron one to be worth porting about 180,000 lines of
client, and keeping two clients in step while the browser and phone stay on web technology?

Known so far: the shell alone feels quicker on Windows (the owner's own judgement, 14 November
2026). Not known: whether that holds for the hard case (a long conversation streaming in), and what
the features Rune can't do will cost. Step 1 answers the first. "Open decisions" lists the second.

## Where it stands

| Piece | State |
|---|---|
| Shell: rail, session list, header, conversation, composer, content panel, status bar | Built, from invented data |
| Tests with no window; every scene drawn on Linux, Windows and macOS in CI | Built |
| Markdown, code blocks, long conversations | Step 1, slice A |
| A real server | Step 1, slice B |
| Everything else | Step 2 |

## How work is cut

- A **slice** goes through every layer and ends in something a person can try. It has a demo
  sentence, a budget, and the files it owns.
- One session builds one slice or lane, from start to merged.
- Budgets are counts: lines of Rust added (tests included) and pull requests. Past the budget by
  half, stop and say why.
- **Step 1 comes first and finishes before Step 2 starts.** Fanning out across layers before one
  thin path works end to end is how a lot of code gets written and little of it works.

## Step 1: the gate

Two slices. Both must be on `main`, with their demo sentences true, before any lane in Step 2
starts. If slice A's numbers say native isn't clearly better, stop there and report.

### Slice A: a conversation that streams

> Open the "Long conversation" session: 2,000 messages of markdown, tables, code blocks and tool
> calls scroll from top to bottom without a hitch while a reply streams in at the end, and
> `docs/measurements/conversation.md` puts the numbers beside the same conversation in Fleet's
> desktop app.

This is the measurement the whole port rests on, so it comes first.

- **Build:** message text as markdown (`rune-kit`'s `Markdown`), highlighted code blocks, only the
  rows on screen built (`virtual_rows`), a streaming tail, a generated long conversation and a
  scripted stream in the mock data (the same every run), and scenes for both.
- **Measure**, on real hardware in a release build: time to open the session, frame times while
  scrolling and while streaming, CPU while streaming, memory. Then the same conversation in Fleet's
  Electron app (a scratch Fleet with a scripted model, `docs/FLEET.md`), with Chromium's own
  tracing. Write down what each number leaves out.
- If the session's machine has no GPU, the commit and layout times from a headless run are still
  valid; frame times are not. Give the owner one command that runs the measurement on their
  machine and writes the result to a file.
- **Owns:** `src/view/conversation.rs` (split out of `sheet.rs` in the first pull request, with the
  message type it draws), the long conversation in `src/mock.rs`, `docs/measurements/`.
- **Budget:** 1,500 lines, 3 pull requests.
- **Stop and report if** scrolling can't hold the display's frame rate on an ordinary laptop, or
  streaming costs more CPU than the web app.

### Slice B: talk to a real Fleet

> Start the app with `--server http://127.0.0.1:<port>` beside a scratch Fleet: the session list
> shows that Fleet's projects and sessions, opening one shows its conversation, a message typed in
> the composer reaches the agent, and the reply appears in the conversation as it is written.

The thin path through every layer. Thin means: text and tool-call rows only, one machine, loopback,
no token.

- **Build:** the `--server` argument; HTTP calls for projects, sessions, prompt and abort; the
  SignalR hub (subscribe to the session list and to the open session, take the snapshot, apply
  events); the reducer, ported as a pure function with the cases from Fleet's own tests that this
  path touches; the state filled from the server instead of `mock.rs` when a server is given.
  With no server, nothing changes: tests and scenes still use the mock data.
- **Check:** against a scratch Fleet with a scripted model, on Linux at least. Keep captured hub
  frames as test fixtures.
- **Owns:** `src/fleet/` (new: HTTP, hub, events, reducer and their tests), the server wiring in
  `src/lib.rs`, `tests/fixtures/`.
- **New crates:** HTTP, WebSocket and JSON. Name each in the pull request with what it adds to a
  clean build. Start from threads and blocking calls (`docs/RUNE.md`, "Work off the UI thread").
- **Budget:** 3,000 lines, 4 pull requests.
- **Starts when** slice A's first pull request (the conversation file and message type) is merged,
  so the two don't rewrite the same type. After that they can run side by side.

## Step 2: lanes

Each lane is one session at a time and owns its folder. Lanes with no "needs" can run side by side
once Step 1 is done. Sizes are the Vue client's lines for the same area (`docs/FLEET.md`); a lane
far bigger than a session splits into slices, each with its own demo sentence, before it starts.

| Lane | Demo sentence | Owns | Vue source | Needs |
|---|---|---|---|---|
| 1. Every event | Every case in Fleet's reducer tests passes here under the same name, and a session with subagents, a question and a failed turn shows the right status at each step. | `src/fleet/reducer*` | `lib/domain-event-reducer.ts`, 74 test cases | |
| 2. Parts of a message | Tool calls open to show their input and output, edits show a diff, reasoning folds, a compaction shows its divider, and a question or permission ask can be answered. | `src/view/conversation/` | `components/session` (20,360) | Lane 1 for asks |
| 3. Composer | Pick the agent, model and effort, attach a file, queue or steer a message while the agent works, and use `/` commands and `@` mentions. | `src/view/composer/` | `components/session` composer files | |
| 4. Session list | Start a new session, add a project folder, rename, archive, filter, and see child sessions under their parent with Fleet's muted lineage lines. | `src/view/sessions/` | `components/sessions` (11,741) | |
| 5. Kit | Buttons, menus, dialogs, selects, tooltips, toasts and the command palette look and behave like Fleet's, and every other lane uses them. | `src/ui/` | `components/ui` (3,615) | Start first: others lean on it |
| 6. Content panel | The tabs: Changes with its diff and review, Files and a file viewer, Progress, Turns, Agents. | `src/view/content/` | `components/canvas` (9,369) | Decisions 1 and 3 for pages, browser, editor |
| 7. Terminal | The drawer opens a working terminal on the session's folder. | `src/view/terminal/` | `components/terminal` (1,206 plus xterm.js) | Decision 2 |
| 8. Settings | Every settings page, under the same menu. | `src/view/settings/` | `components/settings` (17,521) | Lane 5 |
| 9. Other rail views | Workflows, Automations, Analytics, Jobs, Fleet and GitHub, one slice each. | `src/view/<name>/` | their folders | Lane 5 |
| 10. Machines | Connect to a Fleet on another machine with a token, pair a new one, and run sessions on it. | `src/fleet/machines*` | `lib/machines.ts`, `lib/machine-target.ts` | |
| 11. Desktop shell | Double-click the app with no Fleet running: it starts its own server, shows the window, and a second launch brings the first to the front. Tray, notifications, deep links, window position. | `src/desktop/` | `desktop/src` | |
| 12. Packages | A signed installer for macOS, Windows and Linux with the server inside, and updates. | `.github/workflows/`, `packaging/` | `desktop-packages.yml` | Lane 11 |
| 13. Mods | A mod's buttons, chips and panes appear where they do in Fleet. | `src/view/mods/` | `components/mods` (2,140) | Lane 5; decision 1 for pages |
| 14. Reach and keys | A screen reader reads every screen; every Fleet shortcut works; Tab order is right. | all (review pass) | | Last |

## Open decisions

These are the owner's to make. A session that reaches one stops and asks; it doesn't pick.

1. **Web content.** Fleet shows HTML in four places: pages agents show, HTML previews, the browser
   canvas and mod pages. Rune has no web view. Options: (a) open them in a separate system web
   view window beside the app; (b) embed the system web view inside the window (possible, but it
   sits on top of everything Rune draws, so nothing can overlap it); (c) leave those features to
   the Electron and browser clients. Nothing before lane 6 needs the answer.
2. **Terminal.** Build one here from a terminal-state crate and Rune text (the server already owns
   the pty), or leave it out of the native app at first.
3. **File viewer and editor.** Fleet's is CodeMirror. Rune has syntax colours and a diff view but
   no editor with line numbers, folding or find. Read-only viewer first, or not at all?
4. **The web build.** Rune can also build for browsers, and the prototype has the scaffolding. It
   was never seen drawing, CI doesn't build it, and the first networking code will break it.
   Default until decided: don't spend effort keeping it alive.
5. **Licence.** This repository is public and has none, as weave-fleet has none.
6. **Moving Rune's pin.** Rune changes about 40 times a day and breaks its API without notice.
   Suggested: one session a week moves the pin in its own pull request, and no lane moves it.
7. **Catching up with Fleet.** The Vue client keeps changing while this is built. Suggested: after
   each Fleet release, one session reads what changed in `client/` for the areas already ported
   and either ports it or lists it here.
8. **When this replaces Electron.** Not before lanes 1 to 5, 11 and 12 are done and decisions 1
   to 3 are made. Until then both exist and Electron is the one that ships.

## Later

Kept out on purpose, so slices stay thin: the phone layout, themes other than light and dark,
several windows, the board, video, and anything Fleet has behind a Labs switch.
