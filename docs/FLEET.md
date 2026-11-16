# Fleet's server contract

What a native client has to speak, and where each part lives in
[weave-fleet](https://github.com/pgermishuys/weave-fleet). Paths are weave-fleet's, as of v0.72.0;
line numbers move, so search for the names.

This is a map, not a spec. Before you build on a row, read the file it points to. If the two
disagree, the code is right: fix this page in the same pull request.

## The shape of it

A Fleet server is one process (`src/WeaveFleet.Api`, .NET). Every client talks to it in two ways:

- **HTTP + JSON** under `/api/...`, for lists, actions and history.
- **One SignalR hub** at `/hubs/session-events`, for everything live: the conversation as it is
  written, session status, progress.

The Vue client keeps almost no truth of its own. It takes a snapshot from the hub, then applies
events to it with one reducer. A native client does the same.

## Where the server is

| Thing | Where |
|---|---|
| Base URL in the Vue client | `client/src/lib/api-client.ts` (`setApiBase`, then `window.__WEAVE_API_BASE__`, then `VITE_API_BASE_URL`, then the page's own origin) |
| A machine and its addresses | `client/src/lib/machines.ts` (`MachineConnection`: `baseUrl`, `addresses[]`, `token`; `machineUrl`) |
| Which machine a call goes to | `client/src/lib/machine-target.ts` (`MachineTarget`) |
| Default port | 6262 |

`/agent/{token}/...` is a server-side prefix for agent processes. A client never uses it.

## Signing in

| Case | What the client sends |
|---|---|
| Server bound to loopback, client on the same machine | Nothing. Loopback requests are trusted (`src/WeaveFleet.Api/Auth/LoopbackAuthPolicy.cs`). This is how the Electron app works. |
| Another machine, or a server that requires a token | `Authorization: Bearer <token>` on HTTP; `?access_token=<token>` on the hub's WebSocket (`Auth/BearerTokenHandler.cs`). The token is the machine's own or a paired device's. |

Fleet Native takes a token with `--token <t>` or `FLEET_NATIVE_TOKEN` (`fleet::auth`): the header on every HTTP call, `&access_token=` on the hub URL, nothing in
any address the window shows. A 401 or 403 stops the reconnecting and says the Fleet needs a token (or refused this one). `https://` works for both calls and the hub (`wss://`): ureq and the hub socket (`src/fleet/tls.rs`) use the same rustls with the `ring` provider and the webpki
root certificates, so a machine behind `tailscale serve` (a Let's Encrypt certificate) verifies. The app can't be told to trust another certificate or to skip
verification; tests add their own certificate to the roots (`tls::test_roots`, test builds only). Pairing and several machines are still a later lane.

**Where the desktop app keeps a remote machine** (weave-fleet `main`): not in a file on the client. Fleet's desktop app always runs, or finds, a Fleet on this
machine and serves its page from it; the other machines it knows live in that Fleet's own database (`RemoteMachineService`, `GET /api/machines`, tokens only
for the owner), and older versions kept them in the page's `localStorage` (`weave:machines`) inside Electron's profile. Neither is read from here. The token of a
Fleet on another machine is shown on that Fleet: Settings > Machines > This machine (also `fleet.machine.json` next to its database, `accessToken`).

## How the desktop app starts its server

`desktop/src/main.ts`, `server.ts`, `instance.ts`, `paths.ts`:

1. Look for a Fleet already running: `<dataDir>/fleet.instance.json` holds
   `{pid, url, version, databasePath, desktop, startedAt}`.
2. Else start the bundled server: `--urls http://127.0.0.1:<port> --contentRoot ...` with
   `Fleet__Host=127.0.0.1`, `Fleet__Port`, `Fleet__DatabasePath`, `Fleet__AnalyticsDatabasePath`,
   `Fleet__DataProtection__KeyPath`, `Fleet__Desktop__Enabled=true`,
   `ASPNETCORE_ENVIRONMENT=Production`.
3. Poll `GET /readyz` every 250 ms, for up to 60 seconds.
4. Exit code 75 from the server means another Fleet holds the database.
5. The server's stdin is kept open so it exits when the app does.
6. The window then loads the server's own origin.

Until the "desktop shell" lane, this app doesn't start a server: it is given one with
`--server <url>`.

## The hub

| Thing | Value | Where |
|---|---|---|
| Path | `/hubs/session-events` | `src/WeaveFleet.Api/Endpoints/EndpointExtensions.cs`; client `client/src/composables/use-signalr-socket.ts` |
| Protocol | SignalR, JSON (not MessagePack) | `src/WeaveFleet.Api/Program.cs` |
| Hub class | `SessionEventsHub` | `src/WeaveFleet.Api/Hubs/SessionEventsHub.cs` |
| Reconnect delays in the Vue client | 1 s, 2 s, 5 s, 10 s | `use-signalr-socket.ts` |

SignalR's JSON protocol over a WebSocket is small: `POST <hub>/negotiate?negotiateVersion=1`
(answers `{connectionToken, ...}`), connect a WebSocket to `<hub>?id=<connectionToken>`, send the
handshake `{"protocol":"json","version":1}` (the server answers `{}`), and then every frame is a
JSON object ending in the byte `0x1e`. Message types: 1 invocation, 3 completion, 6 ping, 7 close.
Microsoft's "SignalR Hub Protocol" document is the reference. There are no captured frames in
weave-fleet; `tests/fixtures/hub-*.jsonl` here are real ones from a scratch Fleet (v0.72.0), with
the separators dropped so each line is one frame.

What the captures showed (v0.72.0):

- The snapshot (the completion of `SubscribeToSessionAsync`) is several KB and the server sends it
  as a fragmented WebSocket message; a client that doesn't join continuation frames loses its tail.
- Live events come as `Event(topic, eventId, data)` with `eventId` null or negative; `data` is
  `{type, eventId, properties}`. On `session:<id>`: `message.updated` (a user prompt, with
  `info`, `parts` and the `correlationId` of the POST), `turn.started`, `session.status`,
  `message.part.updated`, `message.part.delta.streamed` (word-sized deltas with `offset`),
  `message.updated` (the assistant's `info` only, no parts), `session.updated`, `session.idled`,
  `session.idle`. Some carry the harness's own session id (`ses_...`) in `properties.sessionID`,
  not Fleet's; use the topic for which session it is. On `sessions`: `activity_status`
  (`{sessionId, activityStatus, capabilities}`), repeated several times per change.
- A tool part is `{type:"tool", tool:"bash", callID, state:{status, input, output, metadata, time}}`;
  it arrives as `running`, then `running` with output, then `completed`. The snapshot's copy of the
  same part has no `time` in its state, so a call's duration is known live but not after a reload.
- `GET /api/sessions` rows carry `activityStatus`, `projectName`, `workspaceDirectory`, `branch`,
  `totalTokens` (null when none), `parentSessionId`, `retentionStatus` and `session.time.updated`
  in milliseconds.

Methods the client calls:

| Method | Returns | For |
|---|---|---|
| `SubscribeToSessionAsync(sessionId)` | `SessionSnapshot` | Open a conversation: the snapshot, then live events on `session:{id}` |
| `UnsubscribeFromSessionAsync(sessionId)` | | Leave it |
| `LoadHistoryAsync(sessionId, cursor)` | `SessionHistoryPage` | Older messages, when the snapshot says `hasMore` |
| `SubscribeToSessionsTopicAsync()` | | Live changes to the session list, on topic `sessions` |
| `UnsubscribeFromSessionsTopicAsync()` | | |
| `SetSessionFocusAsync(sessionId, focused)` | | Tell the server which session the user is looking at |
| `SetPresenceAsync(visible, formFactor)` | | Tell the server the window is visible |

The server calls one method on the client: `Event(topic, eventId, data)`, where `data` is
`{ type, eventId?, properties }`.

The snapshot (`client/src/lib/session-snapshot.ts`):
`{session, messages[], delegations[], runningWork?, activityStatus, lastEventId, hasMore, cursor,
isPartial, recap?, context?}`.

After a reconnect the Vue client subscribes again and takes a fresh snapshot. It doesn't replay.

## Events and the reducer

This is the heart of the client, and the part with the best tests.

| Thing | Where |
|---|---|
| The events the server can send (31 wire names) | `src/WeaveFleet.Domain/Events/DomainEvent.cs` (`[JsonDerivedType]`), records in the same folder |
| Their TypeScript shapes | `client/src/lib/domain-events.ts` |
| The reducer | `client/src/lib/domain-event-reducer.ts`: `createSessionStreamState(snapshot)`, `applyDomainEvent(state, event)`, `isStreamWorking`, `isDelegationWaiting`, `withActivityStatus` |
| Its state | `{messages, delegations, runningWork, context, explicitStatus, sessionStatus, lastEventId}` |
| Message and part shapes | `client/src/lib/client-types.ts` (`AccumulatedMessage`; parts: text, reasoning, tool, file, compaction) |
| Its tests (the contract to port) | `client/src/lib/__tests__/domain-event-reducer.test.ts` (56 cases), plus the reopened-session (3), message-ordering (4) and running-work (11) tests beside it |
| Shared fixtures | `tests/contracts/fleet-api-events.json` and its siblings |
| How the Vue client uses it | `client/src/composables/use-session-stream.ts` (subscribe, load history, batch events per frame) |
| The phone app running the same reducer | `mobile/src/fleet/hub.ts` |

Wire names: `session.started`, `session.idled`, `session.deleted`, `session.archived`,
`session.recap`, `session.messaged`, `session.reported`, `turn.started`, `turn.ended`,
`turn.failed`, `message.created`, `message.updated`, `message.part.updated`,
`message.part.delta.streamed`, `delegation.created`, `delegation.updated`, `delegation.completed`,
`files.changed`, `todos.reported`, `files.written`, `canvas.updated`, `canvas.closed`,
`canvas.focused`, `terminal.opened`, `terminal.closed`, `app.updated`, `browser.step`,
`mods.changed`, `mods.runtime`, `mod.ui`, `mod.pane`.

Port the reducer as a pure function with no Rune in it, and port its tests with it, case for case,
under the same names. A reducer test that passes in TypeScript and fails in Rust is a bug in the
port.

## HTTP

All in `src/WeaveFleet.Api/Endpoints/`. Generated types: `client/src/api/generated/schema.d.ts`.
Hand-written types and calls: `client/src/api/client.ts`. A running server lists every route at
`/openapi/v1.json`.

| Call | For | Notes |
|---|---|---|
| `GET /api/projects` | The session list's groups | `ProjectEndpoints.cs` |
| `GET /api/sessions` | The session list | Query: `limit`, `offset`, `statuses`, `projectId`, `retentionStatus`, `tags`. Rows are `SessionListResponse` (`Application/DTOs/SessionDtos.cs`); the status field is `sessionStatus`. |
| `GET /api/sessions/{id}` | One session | |
| `GET /api/sessions/{id}/messages?limit&before` | History over HTTP | The live path is the hub |
| `POST /api/sessions/{id}/prompt` | Send a message | `SendPromptApiRequest`: `text`, `agent`, `model`, `attachments`, `userMessageId`, `correlationId`, `effort`, `delivery` (`"queue"` or `"steer"`), `sessionReferences`, `skills` |
| `POST /api/sessions/{id}/abort` | Interrupt | |
| `GET /api/sessions/{id}/progress` | Todos, plan, subagents | 204 when there are none; live updates arrive as events |
| `PATCH /api/sessions/{id}` | Rename | `{title}` |
| `PATCH /api/sessions/{id}/retention` | Archive | `{retentionStatus}` |
| `POST /api/sessions` | New session | |
| `GET /api/workspace-roots`, `POST /api/workspace-roots` | Folders Fleet may use | A new Fleet refuses a session in a folder until its root is added |

## How big the Vue client is

Lines that aren't tests, as of v0.72.0. Use it to size a lane, not as a target.

| Area (`client/src/`) | Files | Lines |
|---|---|---|
| `components/session` (conversation, composer, tool calls) | 57 | 20,360 |
| `components/settings` | 69 | 17,521 |
| `components/sessions` (the list) | 28 | 11,741 |
| `components/canvas` (content panel) | 21 | 9,369 |
| `components/phone` (not ported) | 49 | 8,240 |
| `components/workflows` | 16 | 6,569 |
| `components/board` | 8 | 3,840 |
| `components/ui` (buttons, menus, dialogs) | 138 | 3,615 |
| `components/automations` | 11 | 3,492 |
| `components/analytics` | 13 | 3,029 |
| `components/layout` | 14 | 2,871 |
| `components/session-context` | 6 | 2,676 |
| `components/mods` | 26 | 2,140 |
| `components/terminal` | 3 | 1,206 |
| `composables` | 166 | 20,724 |
| `lib` | 226 | 27,491 |
| `stores` | 39 | 7,220 |
| `plugins` (GitHub) | 34 | 6,917 |

Design tokens: `client/src/assets/main.css` (`:root` is dark, then light, then the other themes,
then `@theme inline`).

## A scratch Fleet to test against

A second Fleet server started with your real home folder can damage the database of the Fleet you
use, and a test client can send prompts that make agents act. So:

- **Never point a test at a real Fleet, and never start a Fleet server with your real home
  folder.** Give it a new, empty `HOME` (on Linux also `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
  `XDG_CACHE_HOME` and `XDG_STATE_HOME` inside it).
- Bind it to `127.0.0.1` on a port that isn't 6262.
- Stop it when you're done, by the process listening on that port.

Two ways to get one:

1. **A released server.** Download `fleet-v<version>-<linux-x64|osx-arm64|win-x64>` from
   weave-fleet's Releases, unpack it, and run `bin/fleet --port <n>` with the scratch `HOME`.
   The launcher takes the port from `--port`: `Fleet__Port` is ignored by it, and with neither it
   starts on 6262, where a real Fleet lives (it failed to bind here, but check the port you gave).
   `scripts/scratch-fleet.sh` does all of this, with the scripted model.
2. **From source, with a scripted model.** `mobile/e2e/start-fleet.sh` in weave-fleet starts a
   throwaway Fleet whose agent talks to `mobile/e2e/fakellm.py`, a stand-in model that replies from
   a script. Replies are the same every run and cost nothing, which is what a streaming test
   needs. Copy its approach (scratch home, scripted model, add a workspace root, start a session);
   don't run a real model from a test.

A new Fleet refuses `POST /api/sessions` with "Path is outside allowed workspace roots" until you
`POST /api/workspace-roots {"path": "<scratch home>/proj"}`.
