//! The "Long conversation" session: 2,000 invented messages of prose, lists, tables, code and tool
//! calls, the same on every run, and the scripted reply that streams in at its end. The slice A
//! measurements scroll this (`docs/measurements/conversation.md`).
use crate::view::conversation::{Message, Part, ToolState};

pub const TITLE: &str = "Long conversation";
/// Turns in the generated history: a user message and an assistant reply each.
pub const TURNS: usize = 1_000;

const AREAS: [&str; 8] =
    ["image cache", "checkout page", "date picker", "release notes", "map tiles", "onboarding", "search index", "alert rules"];
const FILES: [&str; 6] =
    ["src/store/index.ts", "src/pages/checkout.vue", "src/lib/dates.ts", "src/map/tiles.rs", "docs/notes.md", "tests/alerts.spec.ts"];

pub fn conversation() -> Vec<Message> {
    (0..TURNS)
        .flat_map(|n| {
            let area = AREAS[n % AREAS.len()];
            let ask = format!("Turn {n}: look at the {area} and tell me what you would change first, with the numbers behind it.");
            [Message::user(format!("u{n}"), ask), Message::assistant(format!("a{n}"), reply(n))]
        })
        .collect()
}

/// One assistant reply. Eight shapes in turn, so any screenful holds a mix.
fn reply(n: usize) -> Vec<Part> {
    let id = |k: usize| format!("a{n}-p{k}");
    let area = AREAS[n % AREAS.len()];
    let file = FILES[n % FILES.len()];
    let (a, b, c) = (12 + n % 31, 40 + n * 7 % 53, 3 + n % 9);
    let prose = |k: usize, body: String| Part::text(id(k), body);
    match n % 8 {
        0 => vec![prose(
            0,
            format!(
                "## What I found in the {area}\n\nThe slow part is not where it looks. Of the `{a}` calls per request, **{b}%** repeat \
                 an earlier one, and each repeat costs about {c} ms.\n\n- Read `{file}` once and keep the result\n- Drop the second \
                 lookup, which only checks what the first already knows\n- Add a test that counts the calls, so it stays fixed\n\nThe \
                 change is small, and I would start there."
            ),
        )],
        1 => vec![prose(
            0,
            format!(
                "Here is how the three options compare for the {area}:\n\n| Option | Calls | Time (ms) | Risk |\n|---|--:|--:|---|\n| Keep as is | \
                 {a} | {b} | none |\n| Cache the result | {} | {} | stale reads |\n| Batch the calls | {} | {} | larger change |\n\nCaching \
                 wins on time; batching wins if staleness matters.",
                a / 2,
                b / 3,
                a / 4,
                b / 5
            ),
        )],
        2 => vec![
            prose(0, format!("This is the shape I would give the {area} lookup. It reads once and shares the answer:")),
            prose(
                1,
                format!(
                    "```rust\nuse std::collections::HashMap;\n\n/// Answers for the {area}, by key.\npub struct Lookup {{\n    seen: HashMap<String, u64>,\n}}\n\nimpl Lookup {{\n    \
                     pub fn get(&mut self, key: &str) -> u64 {{\n        // A repeat costs one map read instead of {c} ms.\n        *self.seen.entry(key.to_string()).or_insert_with(|| \
                     key.len() as u64 * {a})\n    }}\n}}\n```"
                ),
            ),
        ],
        3 => vec![
            prose(0, format!("I'll read `{file}` and the test beside it, then make the change.")),
            Part::tool(id(1), "Read", file, ToolState::Plain),
            Part::tool(id(2), "Read", "tests/lookup.spec.ts", ToolState::Plain),
            Part::tool(id(3), "Edit", file, ToolState::Diff(a as u32, c as u32)),
            Part::tool(id(4), "Bash", "bun test tests/lookup.spec.ts", ToolState::Took(c as f32 / 2.0 + 0.3)),
            prose(5, format!("Done: the test passes and the {area} makes {} calls where it made {a}.", a / 3)),
        ],
        4 => vec![
            prose(0, "The settings now look like this:".into()),
            prose(
                1,
                format!(
                    "```json\n{{\n  \"area\": \"{area}\",\n  \"maxCalls\": {a},\n  \"ttlSeconds\": {b},\n  \"retry\": {{ \"count\": {c}, \"backoffMs\": [100, 250, 600] }}\n}}\n```"
                ),
            ),
            prose(2, "Raise `ttlSeconds` and reads get cheaper, but changes take longer to show up.".into()),
        ],
        5 => vec![prose(
            0,
            format!(
                "Steps, in order:\n\n1. Stop the {area} service\n2. Move the old data to `backup/`\n3. Start it again and wait for the first \
                 request\n4. Compare the count with **{a}**\n\n> If the count is higher than before, undo step 2 first. Nothing else needs \
                 undoing."
            ),
        )],
        6 => vec![
            prose(0, format!("To see it for yourself, run this in the {area} folder:")),
            prose(
                1,
                format!(
                    "```sh\ncd ~/code/{}\nbun install\nbun test --rerun-each {c}\nbun run bench -- --calls {a}\n```",
                    area.replace(' ', "-")
                ),
            ),
            prose(2, "It prints one line per run. Look at the last column.".into()),
        ],
        _ => vec![
            prose(
                0,
                format!(
                    "### Summary\n\nThe {area} is **{b}% faster** in the new build. One caveat: the first request after a restart is slower by {c} ms.\n\n| Run | Before | \
                     After |\n|---|--:|--:|\n| First | {} | {} |\n| Median | {b} | {} |\n\n```ts\nconst answer = await lookup.get(key); // one read, shared\n```",
                    b + c,
                    b / 2 + c,
                    b / 2
                ),
            ),
            Part::tool(id(1), "Bash", "bun run bench", ToolState::Took(a as f32 / 4.0)),
        ],
    }
}

/// The reply that streams in at the end of the long conversation: a heading, a list, a table and
/// a code block, so the tail grows through every kind of block.
pub const STREAM_REPLY: &str = "## Plan for the search index\n\nI looked at how the index is built and read. Three things stand out, in \
the order I would fix them:\n\n- **Rebuilds are whole.** Every change rebuilds all `4,200` entries, where `3` changed.\n- **Reads wait on \
writes.** A read during a rebuild blocks for about `180 ms`.\n- **Keys are long.** Each key carries its full path, so the index is \
twice as large as it needs to be.\n\n| Step | Change | Saves |\n|---|---|--:|\n| 1 | Rebuild only what changed | 94% of build time |\n| 2 | \
Read the old index until the new one is ready | 180 ms per read |\n| 3 | Key by a short id | half the memory |\n\nStep one is the one to start with. \
Here is its core:\n\n```rust\npub fn apply(index: &mut Index, changed: &[Entry]) {\n    for entry in changed {\n        // Replace in place; \
everything else keeps its slot.\n        index.slots.insert(entry.id, entry.clone());\n    }\n    index.version += 1;\n}\n```\n\nThe tests that \
cover it are small. I would write them first, then the change, then check the numbers again.";

/// How fast the scripted reply arrives, in characters a second (about 70 tokens a second).
pub const STREAM_CPS: f64 = 280.0;
