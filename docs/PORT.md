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
| Conversation split out of `sheet.rs`, with the message type (`Message`, `Part`) | Done (slice A, PR 1) |
| Markdown, highlighted code, tables, virtual rows, the 2,000-message "Long conversation" and its scripted stream | Done (slice A, PR 2); measurements in `docs/measurements/conversation.md` (PR 3): native numbers measured, real-hardware run still to do |
| A real server: sessions, a conversation (text and tool rows), prompt and abort, live events | Done (slice B): `--server <url>`; reconnect, `LoadHistory` and the rest of the reducer are left |
| Everything else | Step 2 |

## How work is cut

- A **slice** goes through every layer and ends in something a person can try. It has a demo
  sentence, a budget, and the files it owns.
- One session builds one piece (one pull request) of a component, from start to merged. A
  component's planning session splits it into pieces first.
- Budgets are counts: lines of Rust added (tests included) and pull requests. Past the budget by
  half, stop and say why.
- **Step 1 comes first and finishes before Step 2 starts.** Fanning out across layers before one
  thin path works end to end is how a lot of code gets written and little of it works.
- **Step 2 goes one component at a time**, in the order below (the owner's instruction, 14 November
  2026: "logical piece by logical piece"). The next starts when the last is merged and checked.

## Step 1: the gate

Two slices. Both must be on `main`, with their demo sentences true, before any component in Step 2
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
- **Done** (checked on Linux against a scratch Fleet v0.72.0; the demo sentence holds). Left, for
  the components that own them: delegations, running work, context usage and the optimistic prompt in
  the reducer (component 2; its tests are ported case for case for what this path touches, same names);
  older history (`LoadHistoryAsync`, when the snapshot says `hasMore`); `SetPresenceAsync` and
  `SetSessionFocusAsync`; the status of sessions other than the open one beyond `activity_status`;
  https, tokens and several machines (component 17). Rebuilding only the changed messages on each batch was done in slice A, PR 2. The reconnect
  loop is written to the Vue client's delays but not exercised against a dropped connection.
- **Starts when** slice A's first pull request (the conversation file and message type) is merged,
  so the two don't rewrite the same type. After that they can run side by side.

## Step 2: components, one at a time

The goal is **parity with Fleet's desktop app**: everything it does, the native app does, the same
way. The desktop app is split into the logical components below and ported in this order. Each
builds on the ones before it. Sizes are Fleet's Vue lines for the same area, to judge the work,
not a target. A component over about 5,000 Vue lines is built in slices, one session each, in order.

### How a component gets done

Every piece of work gets its own Fleet session, so each session's context stays small (the owner's
instruction, 14 November 2026).

1. **A planning session** reads the component's Vue files (and the stores, composables and `lib/`
   code behind them) and writes `docs/parity/<nn>-<component>.md`: the weave-fleet commit it read;
   every element, action, shortcut, menu item, event and state (empty, loading, error, long text,
   both themes), each with its source file; what the native app already has; what is out of reach
   and why. It ends with the **pieces**: in build order, each one pull request of a few hundred
   lines of Rust (at most about 800), with the checklist rows it covers, the files it touches, what
   it depends on, the tests to port by name, and the pictures that prove it. That file is one pull
   request, and the planning session stops there.
2. **One new session per piece**, one at a time, each starting from the checklist file alone. It
   builds its piece: logic as plain Rust with no Rune in it where it can be, Fleet's own tests
   ported under the same names, views at Fleet's sizes, tokens and wording. It takes the pictures
   beside Fleet's (the same scratch Fleet, the same state, the same window size, both themes: the
   native app next to Fleet's own client, which is what the Electron app shows), committed under
   `shots/parity/<nn>-<component>/`. It checks live against a scratch Fleet with a scripted model
   (`scripts/scratch-fleet.sh`), never a real one. It ticks its rows in the checklist, writes down
   every gap with its reason, and merges when every check is green on top of current `main`.
3. The orchestrator looks at each piece's pictures against its rows before the next piece starts.

Each component ports from **weave-fleet's `main` on the day its planning session starts** and
records that commit in its checklist, so the port tracks Fleet as it is, not as it was.

### The order

| # | Component | Parity means | Vue source (`client/src/`) | Vue lines | State |
|---|---|---|---|---|---|
| | **Milestone 1: daily driver** | At the end, a working day can be run in the native app against the user's own Fleet. | | | |
| 1 | Kit | Buttons, menus, popovers, dialogs, selects, comboboxes, tooltips, toasts, right-click menus and the command palette, looking and behaving like Fleet's. Every later component uses them. | `components/ui`, the palette in `components/layout/AppShell.vue` | 3,600 | Done, with gaps listed in `docs/parity/01-kit.md` |
| 2 | Connect and every event | Finds the Fleet running on this machine with no `--server` (`fleet.instance.json`, `/readyz`, as `desktop/src/instance.ts` does); reconnects; older history; presence and focus; all 64 event types of Fleet's union (the server's 31 domain events and the rest it pushes), with all of Fleet's reducer tests passing here under the same names. | `lib/domain-event-reducer.ts` and its tests, `composables/use-session-stream.ts`, `use-signalr-socket.ts`, `stores/` | 3,900 | Done. Pieces A (`src/fleet/events.rs`: every event type read, a subscriber list), B (`delegations.rs`: delegations and derived status), C (`work.rs`, `context.rs`, `command`/`mod` marks) D (`prompts.rs`: optimistic prompts) E (`topics.rs`: topic set, kept streams, reconnect tail, partial banner) F (`history.rs`: older pages, `LoadHistoryAsync` with a 30 s timeout, the button) G (`presence.rs`: `SetPresenceAsync` every 30 s and on visibility, `SetSessionFocusAsync` settled over 250 ms) and H (`rows.rs`: row fields and the `sessions` patches; `refresh.rs`: debounced refetch, 15 s poll while visible, refetch on focus) and I (`instance.rs`: the instance file, `/readyz`, the pid check, the data folder; `lib.rs` runs it on a thread when there is no `--server`) done, and J (the sweep: reconnect now on focus, 128 of Fleet's 135 reducer-side test cases ported under their names and the other 7 owned by components 3 and 5, a live check against a scratch Fleet) done. What is left is other components' (`docs/parity/02-connect-and-events.md`, "Piece J"). |
| 3 | Conversation parts | Tool cards and their output, diffs, reasoning, shell blocks, compaction, turn folding and retry, interrupted turns, background work, subagent rows, images and the lightbox, browser steps, find in conversation, export to markdown. 2 slices. | `components/session`: `ActivityStream`, `MessageBubble`, `ToolCard`, `ToolOutputBody`, `DiffView`, `ReasoningBlock`, `ShellCommandBlock`, … | 7,300 Planned (`docs/parity/03-conversation-parts.md`): pieces A (tool rows), B (tool card and output), C (diffs) D (reasoning, shell block, compaction divider, recap line) and E (message frame, marks, hover, chips) and F (turn folding logic) and G (fold row, the list from F drawn) and H (failure card and retry bar) and I (working line, interrupted card) and J (scroll: the pin and Jump to latest, older messages near the top, show a message) and K (background strip) done, L (work actions), M (sub-agent rows), N (images, lightbox) and O (browser steps) done, P (find bar) done, Q (find marks and reveal) done, R (export builder) done, S (export dialog) done, T (sweep) done; A done (rows, registry, read folding), B done (card body, output box, colours; Copy waits on Rune), C done (diff parser, rows, collapse, write's 12 lines; native diff rows, not `rune_kit`'s), D done (Thinking folded/open/hidden, gist, shell block, compaction divider with summary, recap line), E done (user bubble, command and file chips, mod and steered marks, copy, time pill; session/skill chips and file links open; images in piece N), F done (`foldTurns`, `withFileStats`, the message list: logic and tests, no view), G done (the fold row and the stream drawn from the fold plan; file-stat fetches open), H done (the failure card, Details, Try again, and the retry bar with its countdown, Try now and Don't retry against the server), I done (the working line in its four variants, with the running call and its time; the card a restart leaves, with Continue and Dismiss against the server), J done (the conversation follows the end by intent and offers Jump to latest, asks for older messages within 100 px of the top and keeps the reader's place, rings and centres a message it is told to show; nothing sends `show-message` yet), K done (the background strip above the composer and its rows: running and just-finished work, the header with its summary and fold), L done (Stop with its refusal line, Output with the 2 s refresh and the tail, Details, Events; Open waits for child sessions in the list), M done (a sub-agent call with a session is one row: kind, task, status word and glyph, Open on hover; a background call says Background until its notice; notices are chips; work Fleet stopped reads stopped; the peer chip's two tests wait for machines), N done (a call's screenshot under it, 120 px tall, fetched on a thread; the lightbox with Esc, scrim and close button; a user's images in their bubble; the sniffing and the limits), O done (the steps a call took in the agent's browser, under it: loaded from `agent-browser` and kept current by `browser.step`; the Browser field's `browser_target`), Q done (every match marked in prose, code and call output, the current one solid; the turn and a closed call's card open for it; the card opening is a flagged difference from Fleet), R done (`conv::export`: the Markdown of a conversation, the key-line count, the file name; no view yet), S done (Export as Markdown from the palette: options, source and preview, loading, failed with Try again, Save .md through the system's save dialog, Copy; the whole history over REST as Fleet does), P done (the find bar: Ctrl/Cmd+F by Fleet's focus rules, query, scope chip, "n of m", stepping with wrap, searching older with Stop and Try again; no palette entry, as in Fleet), T done (every Fleet test case listed with its Rust test or owner, accent list markers and Fleet's code colours, a live run of every scripted-model keyword, closing pictures beside Fleet's; open rows each name an owner in the checklist's "Piece T, done"). **Done, with gaps listed in `docs/parity/03-conversation-parts.md`.** |
| 4 | Questions and permissions | Every ask an agent can raise, answered from the conversation; allowed rules. | `QuestionCard`, `PermissionCard`, `Allowed*`, `PagePickCard` | 1,800 | **Done** (`docs/parity/04-questions-and-permissions.md`; every Fleet test case ported or given an owner, the live run and pictures in `shots/parity/04g-sweep/`; what stays open there has an owner: the card's entrance animation and screen-reader roles to component 26, the Settings and Open page buttons to components 16 and 14). Pieces A (permission card view), B (permission asks live: load, events, answer, keys; `src/fleet/asks.rs`, `src/asks/focus.rs`) C (question card logic and waiting view: `src/asks/question.rs`, `src/view/question.rs`, `src/view/ask_card.rs`) and D (question card live: answer, skip, the answered and skipped rows, built from the message's `question` tool part; `src/fleet/questions.rs`) done, piece E (allowed rules: the store, the list, the chip; `src/asks/rules.rs`, `src/fleet/rules.rs`, `src/view/allowed.rs`, `src/ui/age.rs`) done, piece F (page-pick card: `src/fleet/pages.rs`, `src/view/pagepick.rs`) done, piece G (sweep) done. |
| 5 | Composer | Agent, model and effort pickers; `/` commands and `@` mentions; attachments; queue and steer; the context ring. | `Composer`, `ComposerFrame`, `AutocompletePopup`, `*Selector`, `EffortToggle`, `QueuedMessages`, `ContextRing` | 4,400 |Done (`docs/parity/05-composer.md`; what is left is named there with its owner): pieces A (drafts, status, send routing), B (frame and toolbar) C (choice logic: defaults, effort memory, model groups, number formats), D (pickers on the session's own lists) E (selector dropdown: filter, sections, stars, "N more") F (usage limits: the data, the provider bars) G (the context ring), H (ring popover, Compact now) and I (queue and steer) and J (autocomplete logic: triggers, items, keys, picks, references, skills, file matching; no view) and K (the autocomplete popup, live: lists, file search, slash commands sent as commands) and L (references in the draft: tints, whole-piece editing, picks kept, sent with the prompt) and M (message history on ArrowUp, shell mode run against the server, the `/btw` mode, pill and draft swap; the side panel waits for component 15) and N (image attachments: the picker, paste, drop, chips, the lightbox, `attachments` in the prompt, the queue refusal, put-back after a refusal) done; O (files held until Send and uploaded in order, the note, small text files inline, large pastes with the offer bar and the `LargePastes` setting, terminal-line chips and their blocks) done; piece P (sweep: recent models on send, the popup scrolls to its highlight, the missing tests, a live end-to-end run, closing pictures) done. |
| 6 | Session list | Projects and machines, status glyphs, rename, archive with undo, delete, drafts, child sessions with muted lineage lines, filter, right-click menus. 2 slices. | `components/sessions` | 7,900 Planned (`docs/parity/06-session-list.md`): pieces A (projects, groups, filter data), B (sessions store, selection by id), C (row anatomy and status), D (chips, progress, ring), E (panel states, filter box), F (settings file, folding, project header), G (project menu, rename, move, delete), H (row rename, keys, confirm dialogs), I (archive with Undo, pins), J (Pinned group, bulk selection), K (Archived view), L (row menu), M (lineage, sub-agent rows), N (Open in), O (New project dialog), P (draft row), Q (right panel shell), R (moved to component 17, piece C), S (drag and drop, optional), T (sweep); open. **A done** (projects, groups, filter data). **B done** (sessions store, selection by id). **C done** (row anatomy and status; see its section in the checklist for the rows left open). **D done** (running chip, progress ring and count, PR badge from mock data; see the checklist). **E done** (loading, error and banner, the three empty wordings, Try again, the filter's focus look, New project wording). **F done** (settings file for folded groups and the side panels, project header with focus ring, Pinned and Ungrouped headers; a folded group stays folded while filtering). **G done** (the + on a hovered header, the project menu, rename in place with Enter/Esc/blur/F2, Move up and Move down, the delete dialog with its two cards; requests on a thread, failures in a toast; the + and New session wait for component 7's page). **H done** (rename in place on a row with F2, double-click, Enter, Esc and blur, optimistic with roll-back; the confirm dialog for Clear attachments and Delete session with busy and a retry on failure; Alt+Up/Down, the arrows and shift/ctrl click wait for pieces I, J and M). **I done (16 November 2026)** (Pin and Archive buttons on a hovered row, Restore on an archived one; archive waits 6 s for Undo in one toast with its drain and an error variant; pins optimistic with roll-back, Alt+Up/Down moves one; every call goes to the session's own machine, so another machine's rows have them too; the Pinned group is J's). **J done (16 November 2026)** (the Pinned group above the projects, folded and kept like a project; Ctrl/Cmd-click, Shift ranges and plain clicks while picking, on home's rows and another machine's; checkboxes, the selection bar with Archive and Restore, Esc and the clear button). |
| 17 | Machines | Several machines in the session list, the machine switch, pairing, a node's sessions. Checked in each setup Fleet runs in (one machine; a node and the app on one machine; several machines). **Built right after component 6's piece H** (15 November 2026): the owner's sessions live on another machine that their local Fleet is connected to, and until the native app can read that list and talk to those machines it can't be used for real work. | `lib/machines.ts`, `lib/machine-target.ts`, `MachineSessionsGroup`, `MachineSwitch`, `MachinesSection` | 4,900 | Planned (`docs/parity/17-machines.md`): pieces A (addresses, per-machine tokens), B (reading the machines), C (heading and groups drawn; absorbs 6's piece R), D (a link per machine), E (open and message), F (another machine's rows), G (heading menu, Work here), H (unreachable, diagnosis, other address), I (https), J (Settings → Machines: cards), K (add a machine), L (SSH set-up), M (addresses), N (machine switch), O (sweep); **A done (15 November 2026)**; **B done (16 November 2026)** (the list, each machine's sessions, projects and identity read on threads every 15 s, a cache file; nothing drawn yet); **I done (16 November 2026)** (https calls and the wss hub, one TLS stack, no new crate); **E done (16 November 2026)** (a row of another machine opens its session in place; history, stream, send and permission answers go to that machine; checked live against a second scratch Fleet); **C done (16 November 2026)** (a heading per machine and each other machine's tree in the list, read-only, from a live second Fleet; the menu, opening rows, actions, the tooltip and the Add a machine form are other pieces); **D done (16 November 2026)** (a link per machine, started on first use; every call and the hub of a session go to its machine with its token; live-checked against a second Fleet); **G done (16 November 2026)** (the heading's right-click menu on every machine, New session on a machine, Work here; New project waits for 06's piece O); **H done (16 November 2026)** (another address for a machine that doesn't answer, probed without a token; home's diagnosis as the heading's tooltip; the conversation says Not answering; checked live by stopping a second Fleet; the diagnosis panel is J's), **F done in part (16 November 2026)** (another machine's rows rename, have the menu with Copy path, and its projects rename, each call going to that machine; pin, archive, drag and pick wait for 06's pieces I, J, S); **fix (16 November 2026)**: a Fleet on this machine that asks for a token (bound to `0.0.0.0`) is reached with this machine's own token from its `fleet.machine.json`, no `--token` needed; open. |
| 7 | New session and first run | The new-session composer with its folder, machine, harness, profile, base and GitHub pickers; new project; the welcome and repositories pages; harness setup. 2 slices. | `components/sessions/new-session`, `harness-setup`, `onboarding`, `pages/WelcomePage`, `pages/RepositoriesPage` | 5,300 | Planned (`docs/parity/07-new-session-and-first-run.md`): pieces A (create request, naming, plan line), B (defaults, draft, harness choice), C (the page's data), D (route, page shell, entry points), E (strip, chips, popover shell, starters), F (folder menu: list), G (folder menu: path view), H (folder menu: clone view, warning), I (workspace and base pickers), J (harness, profile, agent, model chips), K (machine picker, More options), L (message box and sending), M (`@`, images, GitHub places), N (setup wizard), O (setup rows), P (Welcome and Repositories), Q (sweep); open. **K done** (the machine chip with its menu: home first, each machine's reachability and last answer, an unreachable one listed but not pickable, Add a machine…; picking moves the draft and keeps the message; the "…" chip with Project, Title and Tags). **J done** (the harness chip with its menu, only with more than one harness enabled; the profile chip with its menu, the summary line of each profile, "No profile" and the default, only for a harness that has profiles; the agent and model chips over the chosen folder's catalog with what Default means, the folder's remembered choice, a pick the folder no longer offers dropped, none when the harness can't list them; the catalog asked on the session's profile and about an existing worktree as it is; the no-harness notice. The chips sit at the foot of the box slot until L builds the frame). **I done** (the workspace chip and menu: current checkout, new worktree, existing worktrees with the last used first and a gone one dropped; the base chip and menu: default first and marked, checked-out branch, search, typed branch, Fetch origin first and why it is off for a local branch, forgotten with the folder; the plan line from the branches, worktrees and naming templates the page reads for the folder). **H done** (the folder menu's clone view: the repository box, suggestions from registered sources only, the Location and "Another folder…", the preview with git's words, the failure and "Use that folder"; a pasted repository and the Clone row clone on a thread with the percent on the chip; picking another folder lets a clone finish without taking over; a clone that fails after the menu was shut reopens it; `POST /api/directories/clone` read as a stream). **G done** (the folder menu's path view: the box with Up, what is in the folder, Tab and Enter finishing a name, `~` only for the machine's own home, creating folders with git or empty and a first branch, the preview, opening a folder outside the locations adds it, the 409 and the first-commit warning panel; the list view's Create row creates too). **F done** (the folder menu's chip, list view and keys). **E done** (the strip with its machine and folder chips, the `ns-chip` and `ns-pop` looks every picker reuses in `src/ui/chip.rs`, the starters line and its menu, the plan line under the chips; the pickers behind the chips are F-K). **D done** (the `NewSession` route, the page's header and empty stage with no content panel, the list's Draft row, and every way in: the button, a project's +, the draft row, the palette rows and Ctrl/Cmd+N, "New session in this folder"; an empty list opens the page; the box and chips are an empty slot until E and L). **C done** (the page's reads: repositories kept per machine, detail, worktrees, locations, folder defaults, inspect and listing, harnesses with `fresh`, profiles, preferences, worktree naming, the harness catalog with `harness.catalog_changed`; threads in `src/fleet/new_session/feed_*.rs`, no view yet). **B done** (the draft store, the remembered choices in the settings file, the harness choice; plain Rust, no view). **A done** (the create request and its body, titles and branch slugs, the plan line, the worktree naming preview, `POST /api/sessions`; plain Rust in `src/fleet/new_session/`, no view yet). Onboarding is cloud-only and not ported. |
| 8 | Session header and context | The header and its actions, fork, session analytics, smart links and the pull request pill. | `SessionDetailHeader`, `SessionActionToolbar`, `ForkSessionDialog`, `components/session-context` | 4,800 | Planned (`docs/parity/08-session-header-and-context.md`): piece A done (header data: status, capabilities, labels; no view); piece B done (the header drawn: title row, meta row, banners); piece C done (rename in the header, tags, copy path, checkout chip with Restore now); piece D done (the toolbar and ⋯ menu from capabilities, abort, archive and restore without Undo, delete with its confirm, error lines, copy session ID, raw export); pieces E (fork dialog, analytics popover), F (smart-link data, pull request state, the row badge from data), G (header context chips and the pill), H (pill popover, Send to agent), I (Context tab: shell and rows), J (smart-link card), K (progress strip), L (sweep); open. |
| 9 | App frame | Every rail entry and badge, the whole status bar (running work, usage limits), notices, report a problem, sign-in and pairing pages. | `components/layout`, `notices`, `report`, `auth`, `skills` dialogs | 6,500 | |
| | **Milestone 2: the workspace** | | | | |
| 10 | Content panel | Tabs, the collapsed right rail, Progress, Turns, Agents (the agent's browser is pictures from the server). | `components/canvas`: `CanvasHost`, `ProgressCanvas`, `TurnsCanvas`, `Agents*`, `AgentTabView` | 3,300 | |
| 11 | Files and file viewer | The file tree, Go to file, a read-only file viewer with highlighting (slice 1); editing and saving (slice 2). | `FilesCanvas`, `FileCanvas`, `GoToFileDialog`, `FileBrowser*`, `LinesBar` | 3,200 | |
| 12 | Changes and review | Changes with diffs, review, find, files changed, find in files. | `ChangesCanvas`, `ChangesReview`, `ChangesFindBar`, `DiffsTray`, `FilesChanged*`, `FindInFilesResults` | 4,500 | |
| 13 | Terminal drawer | Decision 2. Slices 7a, 7b, 7c of `docs/research/terminal.md`: the view, Fleet's terminals, the drawer. | `components/terminal`, `lib/terminal-socket.ts` | 1,200 | |
| 14 | Web content | Decision 1. The page tab, the browser canvas, the Document tab and rendered HTML files in a web view; pages in the conversation and HTML previews in lists as cards. | `PageCanvas`, `BrowserCanvas`, `ConversationPage`, `visual-renderers`, `mods/ModPage` | 2,800 | |
| 15 | Side conversation | The `/btw` panel. | `SideConversationPanel` | 800 | |
| | **Milestone 3: every rail view** | | | | |
| 16 | Settings | Every page under the same menu. 4 slices: General, Appearance, Workspace, Storage, System · Harnesses, profiles, sign-in · Permissions, credentials, memory and Dreaming, skills and tools · Machines, phones, updates, Labs, What's new. | `components/settings` | 17,500 | |
| 18 | Workflows | The library, a workflow's steps, recent runs and the Run box. 2 slices. | `components/workflows` | 6,600 | |
| 19 | Automations | The list and detail, create and edit. | `components/automations` | 3,600 | |
| 20 | Analytics | Overview, Daily, Projects, Models, Sessions; filters and export. | `components/analytics` | 3,000 | |
| 21 | GitHub | Repositories, issues, pull requests. A plugin here as in Fleet: core never imports it; it fills places core offers. 2 slices. | `plugins/builtin/github`, `components/github` | 6,700 | |
| 22 | Mods | A mod's buttons, chips, panes and pages where Fleet shows them. | `components/mods` | 2,100 | |
| 23 | Labs views | Jobs, Fleet (mission control), the board. 2 slices. | `components/jobs`, `mission-control`, `board`, `dashboard` | 6,700 | |
| | **Milestone 4: replaces Electron** | | | | |
| 24 | Desktop shell | Starts its own server when none is running, one instance, tray, menus, notifications, deep links, window position, the shell environment. | weave-fleet's `desktop/src` | 1,200 (TS) | |
| 25 | Packages and updates | Signed installers for Windows, macOS and Linux with the server inside, and self-update. **Self-update is built** (pulled forward from the rolling `latest` release: a status-bar item, one dialog, SHA-256 check, replace in place, restart; `src/update/`); signed installers and the server inside remain. | `.github/workflows/desktop-packages.yml` | | |
| 26 | Parity audit | Every screen beside Fleet's once more; keyboard and screen reader pass; catch up with Fleet's latest release. Then replacing Electron is the owner's call. | | | |

Between milestones, two sessions run on their own: one **catches the finished components up** with
what changed in Fleet since they were ported, and one **moves Rune's pin** (all five revs, alone in
its pull request, every scene looked at again).

## Decisions

Made on 14 November 2026 by the orchestrator, under the owner's go-ahead to complete the port; the
owner can overturn any of them.

1. **Web content: `wry`** (the system web view as a child of the window), from
   `docs/research/web-content.md`. Places that are whole tabs (pages, the browser canvas, the
   Document tab, rendered HTML files) show a live web view, hidden while any Rune overlay is open
   over it. Pages inside the conversation, mod pages in lists and HTML previews in tool cards become
   a card with an Open button that shows the page in the page tab. If the cards turn out not to be
   enough, CEF (off screen, about 380 MB) is the way to live inline pages, as its own component.
2. **Terminal: `alacritty_terminal` + `rune::grid`**, from `docs/research/terminal.md`.
3. **File viewer: read-only first, then editing**, both in component 11.
4. **The web build is dropped.** Desktop only.
5. **Licence**: still the owner's. It doesn't block the port.
6. **Rune's pin** moves between milestones, never inside a component.
7. **Catching up with Fleet**: a catch-up session between milestones (above).
8. **Replacing Electron** is the owner's call after component 26.

## Out of scope

The phone layout and the phone app, the browser build of this app, and Fleet's themes other than
light and dark. They stay with Fleet's web client.
