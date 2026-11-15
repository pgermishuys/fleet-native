//! A two-turn conversation for find's marks: the first turn is over, so it folds, and holds a paragraph and a code block
//! that both say "entry".
use crate::view::conversation::{Message, Part};

pub fn of_marks() -> Vec<Message> {
    let first = "I'll change `apply` so each entry replaces its slot in place and the other entries keep theirs:\n\n```rust\npub fn apply(index: &mut Index, changed: &[Entry]) {\n    for entry in changed {\n        index.slots.insert(entry.id, entry.clone());\n    }\n    index.version += 1;\n}\n```\n\nA changed entry costs one insert; nothing else is touched.";
    vec![
        Message::user("u1", "Make the index update in place."),
        Message::assistant("a1", vec![Part::text("a1-t", first)]),
        Message::user("u2", "Thanks. Does a deleted one work too?"),
        Message::assistant("a2", vec![Part::text("a2-t", "Yes: a deleted slot is removed the same way.")]),
    ]
}
