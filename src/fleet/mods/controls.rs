//! What a mod's controls remember between draws (component 22, piece C): the text of each Input, when it last told its mod, and
//! the Select that is open; and the actions they send. A control is found by its element key (`mod-tree.0.1`), which holds while
//! the tree keeps its shape; a field that is no longer in the tree is let go, sending what it still held first.
use super::input_rate::{Change, InputRate};
use super::points::OnAction;
use super::types::{Action, ActionKind, Surface};
use rune::{TextEdit, Tx};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// One Input: what it holds and how often it speaks.
#[derive(Clone, Default)]
pub struct Field {
    pub edit: TextEdit,
    /// The tree's `value` when it was last read: the field takes a new one, and keeps the user's typing while it stays the same.
    shown: String,
    rate: InputRate,
    pub on_input: Option<String>,
    pub on_submit: Option<String>,
}

#[derive(Default)]
pub struct ModControls {
    fields: BTreeMap<String, Field>,
    /// The `value` each Select was last drawn with.
    shown: BTreeMap<String, String>,
    /// The Select whose list is open.
    pub open_select: Option<OpenSelect>,
    /// What each Select shows after a pick, until the tree's own `value` changes (a native select keeps the pick).
    chosen: BTreeMap<String, String>,
    /// Where a field that isn't there (a redraw took it between frames) writes, so a handler never has nothing to edit.
    spare: TextEdit,
    /// Everything sent, in order (a scene shows it; a test reads it).
    pub sent: Vec<Action>,
    /// Where an action goes: the site's `on_action`, which posts it to the server off the UI thread. None draws and logs only.
    pub sink: Option<OnAction>,
    /// Actions sent and not yet routed: a tool row's controls (`fleet::mods::rows`) are found by their handle in the row views and
    /// posted with their session's machine on the next frame. A pane's would be routed the same way.
    pub pending: Vec<Action>,
    /// The checked tree each tool row's body was last synced with, by its tree key (held, so a replaced tree is never taken for it).
    pub row_seen: BTreeMap<String, std::sync::Arc<Value>>,
}

/// A Select's list while it is open: whose it is, where a pick goes and what it offers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenSelect {
    pub key: String,
    pub handle: Option<String>,
    pub options: Vec<(String, String)>,
    pub current: Option<String>,
}

/// An Input or a Select in a tree: its key, its `value` and the handles it has.
struct Seen {
    key: String,
    select: bool,
    value: String,
    on_input: Option<String>,
    on_submit: Option<String>,
}

/// The keys the tree draws its elements under: `<key>.<index>` all the way down (`view::mods::tree`).
fn find(key: &str, node: &Value, out: &mut Vec<Seen>) {
    let Some(map) = node.as_object() else { return };
    let kind = map.get("type").and_then(Value::as_str);
    if matches!(kind, Some("Input" | "Select")) {
        let handle = |name: &str| map.get("handles").and_then(|h| h.get(name)).and_then(Value::as_str).map(str::to_string);
        let value = map.get("props").and_then(|p| p.get("value")).and_then(Value::as_str).unwrap_or_default();
        out.push(Seen {
            key: key.to_string(),
            select: kind == Some("Select"),
            value: value.to_string(),
            on_input: handle("onInput"),
            on_submit: handle("onSubmit"),
        });
    }
    for (i, child) in map.get("children").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
        find(&format!("{key}.{i}"), child, out);
    }
}

impl ModControls {
    /// The tree under `key` (its root's first child, as `tree` keys it) was drawn anew: new Inputs start, a changed `value`
    /// replaces the text, and an Input that is gone is let go.
    pub fn sync(&mut self, key: &str, tree: &Value) {
        let mut seen = Vec::new();
        find(&format!("{key}.0"), tree, &mut seen);
        let prefix = format!("{key}.");
        let gone: Vec<String> =
            self.fields.keys().filter(|k| k.starts_with(&prefix) && !seen.iter().any(|s| &s.key == *k)).cloned().collect();
        for k in gone {
            self.release(&k);
        }
        let selects: Vec<String> = seen.iter().filter(|s| s.select).map(|s| s.key.clone()).collect();
        self.chosen.retain(|k, _| !k.starts_with(&prefix) || selects.contains(k));
        for s in seen {
            if s.select {
                // A new `value` from the mod replaces the pick; the same one redrawn keeps it.
                let last = self.shown.insert(s.key.clone(), s.value.clone());
                if last.is_some_and(|l| l != s.value) {
                    self.chosen.remove(&s.key);
                }
                continue;
            }
            let field = self.fields.entry(s.key).or_insert_with(|| Field {
                edit: TextEdit::new(s.value.clone()),
                shown: s.value.clone(),
                ..Default::default()
            });
            if field.shown != s.value {
                field.edit.replace_text(s.value.clone());
                field.shown = s.value;
            }
            field.on_input = s.on_input;
            field.on_submit = s.on_submit;
        }
        let open = self.open_select.as_ref().map(|o| o.key.clone());
        if open.as_deref().is_some_and(|k| k.starts_with(&prefix)) && !tree_has(tree, &format!("{key}.0"), open.as_deref()) {
            self.open_select = None;
        }
    }

    /// The tree is gone (the pane closed): every field under `key` is let go.
    pub fn clear(&mut self, key: &str) {
        let prefix = format!("{key}.");
        let all: Vec<String> = self.fields.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        for k in all {
            self.release(&k);
        }
        self.shown.retain(|k, _| !k.starts_with(&prefix));
        self.chosen.retain(|k, _| !k.starts_with(&prefix));
        if self.open_select.as_ref().is_some_and(|o| o.key.starts_with(&prefix)) {
            self.open_select = None;
        }
    }

    fn release(&mut self, key: &str) {
        let Some(mut field) = self.fields.remove(key) else { return };
        if let (Some(handle), Some(value)) = (field.on_input.take(), field.rate.flush()) {
            self.send(Action { handle, kind: ActionKind::Input, value: Some(value) });
        }
    }

    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields.get(key)
    }

    pub fn edit_mut(&mut self, key: &str) -> Option<&mut TextEdit> {
        self.fields.get_mut(key).map(|f| &mut f.edit)
    }

    /// The field's text for the runtime to write into: the spare one when the field is gone.
    pub fn edit_or_spare(&mut self, key: &str) -> &mut TextEdit {
        match self.fields.contains_key(key) {
            true => &mut self.fields.get_mut(key).expect("checked").edit,
            false => &mut self.spare,
        }
    }

    pub fn open(key: &str, handle: Option<String>, options: Vec<(String, String)>, current: Option<String>) -> OpenSelect {
        OpenSelect { key: key.to_string(), handle, options, current }
    }

    pub fn chosen(&self, key: &str) -> Option<&str> {
        self.chosen.get(key).map(String::as_str)
    }

    /// A Select's option was picked: it shows the pick, and a pick that changes the value is sent (`select`'s `change`).
    pub fn picked(&mut self, key: &str, handle: Option<&str>, value: &str) {
        let before = self.open_select.take().and_then(|o| o.current);
        if before.as_deref() == Some(value) {
            return;
        }
        self.chosen.insert(key.to_string(), value.to_string());
        if let Some(handle) = handle {
            self.select(handle, value);
        }
    }

    pub fn send(&mut self, action: Action) {
        if let Some(sink) = &self.sink {
            sink(&action, Surface::Desktop);
        }
        self.pending.push(action.clone());
        self.sent.push(action);
    }

    /// A number that moves when what the controls under `key` show moves (the text of a field, a pick, an open list): part of a
    /// row's memo key, since the row is built once and reused.
    pub fn stamp(&self, key: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let prefix = format!("{key}.");
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for (k, f) in self.fields.range(prefix.clone()..).take_while(|(k, _)| k.starts_with(&prefix)) {
            (k, &f.edit.text).hash(&mut hash);
        }
        for (k, v) in self.chosen.range(prefix.clone()..).take_while(|(k, _)| k.starts_with(&prefix)) {
            (k, v).hash(&mut hash);
        }
        self.open_select.as_ref().filter(|o| o.key.starts_with(&prefix)).map(|o| &o.key).hash(&mut hash);
        hash.finish()
    }

    /// The Input's text changed (`onInput`): the first change goes now, later ones at most four a second.
    pub fn typed(&mut self, key: &str, now: f64, tx: &mut Tx) {
        let Some(field) = self.fields.get_mut(key) else { return };
        let Some(handle) = field.on_input.clone() else { return };
        let text = field.edit.text.clone();
        match field.rate.changed(now, &text) {
            Change::Send(value) => self.send(Action { handle, kind: ActionKind::Input, value: Some(value) }),
            Change::Hold { wake_in: Some(wait) } => tx.wake_after(Duration::from_secs_f64(wait)),
            Change::Hold { wake_in: None } => {}
        }
    }

    /// Enter or the submit button (`onSubmit`) with what the field holds.
    pub fn submit(&mut self, key: &str) {
        let Some(field) = self.fields.get(key) else { return };
        if let Some(handle) = field.on_submit.clone() {
            let value = Some(field.edit.text.clone());
            self.send(Action { handle, kind: ActionKind::Submit, value });
        }
    }

    pub fn press(&mut self, handle: &str) {
        self.send(Action { handle: handle.to_string(), kind: ActionKind::Press, value: None });
    }

    pub fn select(&mut self, handle: &str, value: &str) {
        self.send(Action { handle: handle.to_string(), kind: ActionKind::Select, value: Some(value.to_string()) });
    }

    /// The clock moved: the held values whose window has ended go now. Asks for a wake for the ones still waiting.
    pub fn keep(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut out = Vec::new();
        for field in self.fields.values_mut() {
            if let (Some(handle), Some(value)) = (field.on_input.clone(), field.rate.due(now)) {
                out.push(Action { handle, kind: ActionKind::Input, value: Some(value) });
            }
            if let Some(wait) = field.rate.wait(now) {
                tx.wake_after(Duration::from_secs_f64(wait.max(0.001)));
            }
        }
        let any = !out.is_empty();
        for a in out {
            self.send(a);
        }
        any
    }
}

/// Whether the node at `key` or below it has the element keyed `want`.
fn tree_has(node: &Value, key: &str, want: Option<&str>) -> bool {
    if Some(key) == want {
        return true;
    }
    node.get("children")
        .and_then(Value::as_array)
        .is_some_and(|kids| kids.iter().enumerate().any(|(i, c)| tree_has(c, &format!("{key}.{i}"), want)))
}
