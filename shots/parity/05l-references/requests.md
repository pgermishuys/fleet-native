# Request bodies, from `fleet/tests/references.rs` (a fake server, not a scratch Fleet)

Draft: `use $tidy-docs with @cache-notes and $nope now`, with `@cache-notes` picked, `tidy-docs` a skill of the session.

`POST /api/sessions/{id}/prompt`
```json
{"text":"use $tidy-docs with @cache-notes and $nope now","correlationId":"prompt-…",
 "sessionReferences":[{"token":"@cache-notes","sessionId":"other-session"}],
 "skills":["tidy-docs"]}
```
A busy session queues it: `POST /api/sessions/{id}/queue` carries the same `sessionReferences` and `skills` with `kind: "prompt"`.
A draft with neither leaves both keys out.
