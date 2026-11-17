//! The sign-in flow a provider's Sign in opens (`HarnessSignInFlow.vue`, `lib/harness-sign-in.ts`): the methods a provider offers,
//! the fields each asks, what is sent, and the browser attempt that is followed until it ends. This file holds the plain rules and
//! the state; `flow_actions.rs` holds what talks to the machine.
use super::Provider;
use rune::TextEdit;
use serde_json::{Map, Value};

/// Answers as the form holds them, by field key (`SignInAnswers`).
pub type Answers = Map<String, Value>;

/// One choice of a field with options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opt {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// A condition on another field's answer: it is (`eq`) or isn't (`neq`) this.
#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub key: String,
    pub equals: bool,
    pub value: Value,
}

/// A field of a method (`HarnessSignInField`).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub hidden: bool,
    pub placeholder: Option<String>,
    pub default: Value,
    pub options: Vec<Opt>,
    pub when: Vec<Cond>,
    pub url: Option<String>,
}

impl Field {
    pub fn parse(v: &Value) -> Option<Field> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let options = v
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|o| {
                let get = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
                Some(Opt { value: get("value")?, label: get("label").unwrap_or_default(), description: get("description") })
            })
            .collect();
        let when = v
            .get("when")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| {
                Some(Cond {
                    key: c.get("key")?.as_str()?.to_string(),
                    equals: c.get("op").and_then(Value::as_str) == Some("eq"),
                    value: c.get("value").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        Some(Field {
            key: text("key")?,
            kind: text("type").unwrap_or_else(|| "string".into()),
            title: text("title"),
            description: text("description").filter(|d| !d.is_empty()),
            required: v.get("required").and_then(Value::as_bool) == Some(true),
            hidden: v.get("hidden").and_then(Value::as_bool) == Some(true),
            placeholder: text("placeholder"),
            default: v.get("default").cloned().unwrap_or(Value::Null),
            options,
            when,
            url: text("url"),
        })
    }

    /// What the form calls it: its title, else its key.
    pub fn name(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.key)
    }

    pub fn is_number(&self) -> bool {
        self.kind == "number" || self.kind == "integer"
    }

    /// A box to type in: not a choice, a tick or a link.
    pub fn is_typed(&self) -> bool {
        !matches!(self.kind.as_str(), "boolean" | "multiselect" | "external") && !(self.kind == "string" && !self.options.is_empty())
    }
}

/// A way to sign in (`HarnessSignInMethod`): `key`, `oauth`, `command` or `env`.
#[derive(Clone, Debug, PartialEq)]
pub struct Method {
    pub kind: String,
    pub id: Option<String>,
    pub label: String,
    pub fields: Vec<Field>,
    /// For `command`: what the harness would run.
    pub command: Vec<String>,
    /// For `env`: the variables it reads.
    pub variables: Vec<String>,
}

impl Method {
    pub fn parse(v: &Value) -> Option<Method> {
        let list = |k: &str| -> Vec<String> {
            v.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect()
        };
        Some(Method {
            kind: v.get("type")?.as_str()?.to_string(),
            id: v.get("id").and_then(Value::as_str).map(str::to_string),
            label: v.get("label").and_then(Value::as_str).unwrap_or_default().to_string(),
            fields: v.get("fields").and_then(Value::as_array).into_iter().flatten().filter_map(Field::parse).collect(),
            command: list("command"),
            variables: list("environmentVariables"),
        })
    }

    /// `methodKey`: unique within its provider.
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind, self.id.as_deref().unwrap_or(""))
    }
}

/// `methodsToOffer`: in the harness's order with the environment last, since it is advice, not a way to sign in from here.
pub fn methods_to_offer(provider: &Provider) -> Vec<Method> {
    let mut methods: Vec<Method> = provider.methods.iter().filter_map(Method::parse).collect();
    methods.sort_by_key(|m| m.kind == "env");
    methods
}

/// `initialAnswers`: each field's default, or an empty answer of its kind.
pub fn initial_answers(fields: &[Field]) -> Answers {
    let mut answers = Answers::new();
    for f in fields {
        let v = match f.kind.as_str() {
            "boolean" => Value::Bool(f.default.as_bool().unwrap_or(false)),
            "multiselect" => {
                let kept = f.default.as_array().map(|a| a.iter().map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_string)));
                Value::Array(kept.into_iter().flatten().map(Value::String).collect())
            }
            "number" | "integer" => {
                if f.default.is_number() {
                    f.default.clone()
                } else {
                    Value::String(String::new())
                }
            }
            "external" => continue,
            _ => Value::String(
                f.default.as_str().map(str::to_string).unwrap_or_else(|| f.options.first().map(|o| o.value.clone()).unwrap_or_default()),
            ),
        };
        answers.insert(f.key.clone(), v);
    }
    answers
}

fn holds(c: &Cond, answers: &Answers) -> bool {
    let same = answers.get(&c.key) == Some(&c.value);
    if c.equals { same } else { !same }
}

/// `visibleFields`: not hidden, and every condition holds for the answers so far.
pub fn visible_fields<'a>(fields: &'a [Field], answers: &Answers) -> Vec<&'a Field> {
    fields.iter().filter(|f| !f.hidden && f.when.iter().all(|c| holds(c, answers))).collect()
}

fn is_empty(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(_) => false,
    }
}

/// `missingAnswer`: the first field that is needed and has no answer, by the name the form shows.
pub fn missing_answer(fields: &[Field], answers: &Answers) -> Option<String> {
    visible_fields(fields, answers)
        .into_iter()
        .find(|f| f.required && f.kind != "external" && is_empty(answers.get(&f.key)))
        .map(|f| f.name().to_string())
}

/// `answersToSend`: the asked fields' answers and the hidden fields' defaults. Numbers go as numbers.
pub fn answers_to_send(fields: &[Field], answers: &Answers) -> Answers {
    let asked: Vec<&str> = visible_fields(fields, answers).iter().map(|f| f.key.as_str()).collect();
    let mut sent = Answers::new();
    for f in fields {
        if f.kind == "external" {
            continue;
        }
        if f.hidden {
            if !f.default.is_null() {
                sent.insert(f.key.clone(), f.default.clone());
            }
            continue;
        }
        let value = answers.get(&f.key);
        if !asked.contains(&f.key.as_str()) || is_empty(value) {
            continue;
        }
        let value = value.cloned().unwrap_or(Value::Null);
        sent.insert(f.key.clone(), if let (true, Value::String(s)) = (f.is_number(), &value) { number(s) } else { value });
    }
    sent
}

/// `Number(text)`: a whole number goes as one; what isn't a number goes as null, as JSON has no NaN.
fn number(text: &str) -> Value {
    let text = text.trim();
    if let Ok(n) = text.parse::<i64>() {
        return Value::from(n);
    }
    text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number)
}

/// `codeToEnter`: the code a device sign-in shows ("Enter code: ABCD-1234"), to copy. The first "code:" followed by four or more
/// letters, digits or dashes that start with a letter or digit.
pub fn code_to_enter(instructions: &str) -> Option<String> {
    let lower = instructions.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("code:") {
        let rest = instructions[from + at + 5..].trim_start();
        let word: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        if word.len() >= 4 && !word.starts_with('-') {
            return Some(word);
        }
        from += at + 5;
    }
    None
}

/// Whether a machine key is another machine than the one this window is on.
pub fn is_remote(machine: &str) -> bool {
    machine != crate::fleet::settings::HOME_MACHINE_KEY
}

/// A browser sign-in under way (`HarnessSignInAttempt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub id: String,
    pub url: String,
    pub instructions: String,
    pub needs_code: bool,
    pub callback: Option<String>,
    /// When it runs out, as the clock time the page shows ("2:34 PM").
    pub expires: String,
}

impl Attempt {
    pub fn parse(body: &str) -> Option<Attempt> {
        let v: Value = serde_json::from_str(body).ok()?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let expires = text("expiresAt")
            .and_then(|t| crate::fleet::work::parse_iso_ms(&t))
            .map(|ms| {
                let (.., h, m) = crate::conv::time::civil(ms, crate::conv::time::local_offset_secs());
                crate::conv::time::clock(h, m)
            })
            .unwrap_or_default();
        Some(Attempt {
            id: text("id")?,
            url: text("url")?,
            instructions: text("instructions").unwrap_or_default(),
            needs_code: v.get("needsCode").and_then(Value::as_bool) == Some(true),
            callback: text("callbackAddress").filter(|c| !c.is_empty()),
            expires,
        })
    }
}

/// What the flow keeps while a provider's Sign in is open. A key lives here only until it is sent, and the text boxes that hold
/// one are secure ones that wipe themselves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flow {
    pub harness: String,
    pub harness_name: String,
    pub provider: String,
    pub provider_name: String,
    /// `methodKey` of the method shown.
    pub method: String,
    pub answers: Answers,
    /// The boxes of the typed fields, in field order, by key.
    pub edits: Vec<(String, TextEdit)>,
    pub key: TextEdit,
    pub error: Option<String>,
    pub busy: bool,
    pub attempt: Option<Attempt>,
    pub code: TextEdit,
    pub landed: TextEdit,
    pub paste_open: bool,
    /// The browser that shows the provider's page is on another machine than Fleet, so its callback to `localhost` can't reach
    /// Fleet by itself (`isRemoteBrowser`; for the native app, the Settings machine isn't this one: `docs/parity/16-settings.md`).
    pub remote: bool,
    /// The field whose choices are open.
    pub choosing: Option<String>,
    /// When "Copied" ends.
    pub copied_until: Option<f64>,
    /// When the attempt is looked at next, and whether a look is out.
    pub poll_due: Option<f64>,
    pub checking: bool,
}

impl Eq for Flow {}

impl Flow {
    /// A flow for `provider`, on its first method.
    pub fn new(harness: &str, harness_name: &str, provider: &Provider) -> Flow {
        let mut flow = Flow {
            harness: harness.to_string(),
            harness_name: harness_name.to_string(),
            provider: provider.id.clone(),
            provider_name: provider.name.clone(),
            key: TextEdit::new(String::new()).secure(),
            ..Flow::default()
        };
        if let Some(first) = methods_to_offer(provider).first() {
            flow.pick(first);
        }
        flow
    }

    /// Another method: its fields start again and the error goes (`watch(method)`).
    pub fn pick(&mut self, method: &Method) {
        self.method = method.key();
        self.answers = initial_answers(&method.fields);
        self.edits = method
            .fields
            .iter()
            .filter(|f| f.is_typed())
            .map(|f| {
                let start = match self.answers.get(&f.key) {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => String::new(),
                };
                (f.key.clone(), TextEdit::new(start))
            })
            .collect();
        self.choosing = None;
        self.error = None;
    }

    /// The answers as they stand: the ticks and choices, and what the boxes hold.
    pub fn current(&self) -> Answers {
        let mut answers = self.answers.clone();
        for (key, edit) in &self.edits {
            answers.insert(key.clone(), Value::String(edit.text.clone()));
        }
        answers
    }

    pub fn edit_index(&self, key: &str) -> Option<usize> {
        self.edits.iter().position(|(k, _)| k == key)
    }

    /// `toggleChoice`.
    pub fn toggle_choice(&mut self, field: &str, value: &str, on: bool) {
        let mut now: Vec<Value> = self.answers.get(field).and_then(Value::as_array).cloned().unwrap_or_default();
        now.retain(|v| v.as_str() != Some(value));
        if on {
            now.push(Value::String(value.to_string()));
        }
        self.answers.insert(field.to_string(), Value::Array(now));
    }
}
