//! The access token for a Fleet that asks for one (a machine reached over the network, or one started with
//! `--require-token`): `Authorization: Bearer <token>` on every HTTP call and `access_token=<token>` on the hub's
//! WebSocket (`docs/FLEET.md`, "Signing in"; `Auth/BearerTokenHandler.cs`).
//!
//! The token is held here and nowhere else: it is never printed, never part of an address that is shown, and the
//! `Debug` of everything else in the app can't reach it.
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

/// An access token. Its `Debug` hides it, so a token inside any struct that derives `Debug` can't reach a log or a
/// panic message; the only way to the text is `expose`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A token from what was typed or sent; blank is no token.
    pub fn new(text: &str) -> Option<Token> {
        let text = text.trim();
        (!text.is_empty()).then(|| Token(text.to_string()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(..)")
    }
}

/// What the app holds: the token it was given for the Fleet it connects to (home), and one per other machine, each
/// under the machine's address. A token goes only to the address it is held for.
struct Tokens {
    given: Option<Token>,
    home: Option<String>,
    by_authority: Option<HashMap<String, Token>>,
}

static TOKENS: RwLock<Tokens> = RwLock::new(Tokens { given: None, home: None, by_authority: None });

/// `host:port` of an address, lower case, with the scheme's default port filled in, so `https://a.ts.net` and
/// `https://a.ts.net:443/` are the same place. A user name in the address is dropped. None when there is no host.
pub fn authority_of(url: &str) -> Option<String> {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => ("http".to_string(), url),
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a).to_ascii_lowercase();
    let (host, port) = match authority.rfind(':') {
        Some(i) if !authority[i..].contains(']') => (&authority[..i], Some(&authority[i + 1..])),
        _ => (authority.as_str(), None),
    };
    if host.is_empty() {
        return None;
    }
    let default = if scheme == "https" || scheme == "wss" { "443" } else { "80" };
    Some(format!("{host}:{}", port.filter(|p| !p.is_empty()).unwrap_or(default)))
}

fn tokens() -> std::sync::RwLockReadGuard<'static, Tokens> {
    TOKENS.read().unwrap_or_else(|e| e.into_inner())
}

fn tokens_mut() -> std::sync::RwLockWriteGuard<'static, Tokens> {
    TOKENS.write().unwrap_or_else(|e| e.into_inner())
}

/// Uses `token` as the one for the Fleet the app connects to (home); an empty one clears it.
pub fn set(token: Option<String>) {
    tokens_mut().given = token.and_then(|t| Token::new(&t));
}

/// Says which address is home, so the token given with `--token` goes there and nowhere else.
pub fn bind_home(base: &str) {
    tokens_mut().home = authority_of(base);
}

/// Uses `token` for the machine at `base`; none (or an empty one) forgets it.
pub fn set_for(base: &str, token: Option<&str>) {
    let Some(authority) = authority_of(base) else { return };
    match token.and_then(Token::new) {
        Some(t) => tokens_mut().by_authority.get_or_insert_with(HashMap::new).insert(authority, t),
        None => tokens_mut().by_authority.get_or_insert_with(HashMap::new).remove(&authority),
    };
}

/// The token for the address `base`: the one held for it, else home's when `base` is home.
pub fn token_for(base: &str) -> Option<Token> {
    let authority = authority_of(base)?;
    let held = tokens();
    held.by_authority
        .as_ref()
        .and_then(|m| m.get(&authority))
        .cloned()
        .or_else(|| (held.home.as_deref() == Some(authority.as_str())).then(|| held.given.clone()).flatten())
}

/// Whether a token was given for home (not what it is).
pub fn is_set() -> bool {
    tokens().given.is_some()
}

/// The header value for a token, or nothing when there is none.
pub fn bearer(token: Option<&str>) -> Option<String> {
    token.map(|t| format!("Bearer {t}"))
}

/// What the hub's URL adds for a token: `&access_token=<escaped>`, or nothing.
pub fn hub_query(token: Option<&str>) -> String {
    token.map(|t| format!("&access_token={}", super::http::escape(t))).unwrap_or_default()
}

/// The hub URL's token part for the machine at `base`.
pub fn hub_query_for(base: &str) -> String {
    hub_query(token_for(base).as_ref().map(Token::expose))
}

/// Adds the bearer header to every request of an agent, picking the token by the address the request is about to
/// go to. Another machine's request never carries home's token, and the header is never copied to a redirect's target.
pub fn middleware(
    mut request: ureq::http::Request<ureq::SendBody>,
    next: ureq::middleware::MiddlewareNext,
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let token = token_for(&request.uri().to_string());
    if let Some(value) = bearer(token.as_ref().map(Token::expose)).and_then(|v| ureq::http::HeaderValue::from_str(&v).ok()) {
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
