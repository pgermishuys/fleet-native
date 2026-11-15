//! The access token for a Fleet that asks for one (a machine reached over the network, or one started with
//! `--require-token`): `Authorization: Bearer <token>` on every HTTP call and `access_token=<token>` on the hub's
//! WebSocket (`docs/FLEET.md`, "Signing in"; `Auth/BearerTokenHandler.cs`).
//!
//! The token is held here and nowhere else: it is never printed, never part of an address that is shown, and the
//! `Debug` of everything else in the app can't reach it.
use std::sync::RwLock;

static TOKEN: RwLock<Option<String>> = RwLock::new(None);

/// Uses `token` for every call from now on; an empty one clears it.
pub fn set(token: Option<String>) {
    let token = token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    *TOKEN.write().unwrap_or_else(|e| e.into_inner()) = token;
}

/// Whether a token was given (not what it is).
pub fn is_set() -> bool {
    TOKEN.read().unwrap_or_else(|e| e.into_inner()).is_some()
}

fn current() -> Option<String> {
    TOKEN.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The header value for a token, or nothing when there is none.
pub fn bearer(token: Option<&str>) -> Option<String> {
    token.map(|t| format!("Bearer {t}"))
}

/// What the hub's URL adds for a token: `&access_token=<escaped>`, or nothing.
pub fn hub_query(token: Option<&str>) -> String {
    token.map(|t| format!("&access_token={}", super::http::escape(t))).unwrap_or_default()
}

/// The hub URL's token part for the token in use.
pub fn current_hub_query() -> String {
    hub_query(current().as_deref())
}

/// Adds the bearer header to every request of an agent. Reads the token per request, so it is set once and used by every agent.
pub fn middleware(
    mut request: ureq::http::Request<ureq::SendBody>,
    next: ureq::middleware::MiddlewareNext,
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    if let Some(value) = bearer(current().as_deref()).and_then(|v| ureq::http::HeaderValue::from_str(&v).ok()) {
        let mut value = value;
        value.set_sensitive(true);
        request.headers_mut().insert(ureq::http::header::AUTHORIZATION, value);
    }
    next.handle(request)
}

/// Whether a failure says the server refused for want of (or a wrong) token: a 401 or 403.
pub fn is_refusal(why: &str) -> bool {
    why.contains("http status: 401") || why.contains("http status: 403")
}

/// What the status bar says when a Fleet refuses.
pub fn refused_note(base: &str) -> String {
    if is_set() {
        format!("Fleet at {base} refused the access token. Check it and run again with --token <token> or FLEET_NATIVE_TOKEN.")
    } else {
        format!(
            "Fleet at {base} needs an access token. Run again with --token <token> or FLEET_NATIVE_TOKEN; Fleet shows it in Settings > Machines > This machine."
        )
    }
}

/// Takes `--token <t>` (or `--token=<t>`) out of the arguments.
pub fn split_token(args: impl Iterator<Item = String>) -> (Option<String>, Vec<String>) {
    let (mut token, mut rest) = (None, Vec::new());
    let mut args = args;
    while let Some(arg) = args.next() {
        if arg == "--token" {
            token = args.next();
        } else if let Some(t) = arg.strip_prefix("--token=") {
            token = Some(t.to_string());
        } else {
            rest.push(arg);
        }
    }
    (token, rest)
}

/// `--token` first, then `FLEET_NATIVE_TOKEN`.
pub fn explicit_token(arg: Option<String>, get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    arg.or_else(|| get("FLEET_NATIVE_TOKEN")).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}
