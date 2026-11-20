//! Invented API keys for Settings → API keys when no Fleet is connected, so the page, its scenes and its tests come out the same
//! every time. A hint is what a server keeps of a key; these are made up.
use crate::fleet::settings::credentials::Credential;

fn key(id: &str, label: &str, namespace: &str, hint: &str) -> Credential {
    Credential { id: id.into(), label: label.into(), namespace: namespace.into(), kind: "api-key".into(), hint: hint.into() }
}

pub fn keys() -> Vec<Credential> {
    vec![key("cred-1", "Build key", "anthropic", "...0001"), key("cred-2", "Review key", "openai", "...0002")]
}
