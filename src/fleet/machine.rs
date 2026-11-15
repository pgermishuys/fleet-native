//! A machine: another Fleet this app can reach, where it is and how it is told apart. Ports weave-fleet's
//! `lib/machines.ts` (the record, `normalizeBaseUrl`), `lib/machine-routes.ts` (addresses, `kindOf`, `moveAddress`),
//! `lib/machine-target.ts` (the target) and `lib/pairing-link.ts` / `lib/pairing.ts` (what is typed to add one).
//!
//! No I/O here. The access token of a machine is held by `auth`, keyed by address, and the `token` a machine record
//! carries is a `Token`, whose `Debug` prints nothing.
use super::auth::Token;
use serde_json::Value;

/// How an address is reached, from its host alone (the same rule home uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tailnet,
    TailnetIp,
    Lan,
    Hostname,
    Other,
}

impl Kind {
    /// What Settings calls each kind (`KIND_LABELS`).
    pub fn label(self) -> &'static str {
        match self {
            Kind::Tailnet => "Tailscale",
            Kind::TailnetIp => "Tailscale IP",
            Kind::Lan => "Local network",
            Kind::Hostname => "Host name",
            Kind::Other => "Address",
        }
    }

    /// Whether an address goes over Tailscale.
    pub fn is_tailscale(self) -> bool {
        matches!(self, Kind::Tailnet | Kind::TailnetIp)
    }

    fn parse(text: &str) -> Kind {
        match text {
            "tailnet" => Kind::Tailnet,
            "tailnet-ip" => Kind::TailnetIp,
            "lan" => Kind::Lan,
            "hostname" => Kind::Hostname,
            _ => Kind::Other,
        }
    }
}

/// What came of home's last try of an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressResult {
    Ok,
    NoAnswer,
    OtherMachine,
}

/// One of a machine's addresses, as home lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    /// Base URL without a trailing slash.
    pub url: String,
    pub kind: Kind,
    /// The machine reported it; false for the address it was added with.
    pub learned: bool,
    pub last_result: Option<AddressResult>,
    pub last_tried_at: Option<String>,
}

impl Address {
    fn from_json(v: &Value) -> Option<Address> {
        let url = v.get("url")?.as_str()?.to_string();
        let kind = v.get("kind").and_then(Value::as_str).map_or_else(|| kind_of(&url), Kind::parse);
        let last_result = match v.get("lastResult").and_then(Value::as_str) {
            Some("ok") => Some(AddressResult::Ok),
            Some("no-answer") => Some(AddressResult::NoAnswer),
            Some("other-machine") => Some(AddressResult::OtherMachine),
            _ => None,
        };
        Some(Address {
            url,
            kind,
            learned: v.get("learned").and_then(Value::as_bool).unwrap_or(false),
            last_result,
            last_tried_at: v.get("lastTriedAt").and_then(Value::as_str).map(str::to_string),
        })
    }
}

/// A machine the app has been given (`MachineConnection`): never home, which the app is already connected to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    /// The machine's own id, stable across restarts, port changes and renames. Also its key in every per-machine map.
    pub id: String,
    pub name: String,
    /// The address home uses; requests go to `base_url()`.
    pub base_url: String,
    /// Every address the machine can be reached at, in the order to try them; empty from an older home.
    pub addresses: Vec<Address>,
    pub token: Option<Token>,
    pub os: Option<String>,
    pub added_at: String,
    /// `user@host` when home reaches it over SSH.
    pub ssh_destination: Option<String>,
    /// Whether the SSH tunnel is up; none when it isn't an SSH machine.
    pub tunnel_up: Option<bool>,
}

impl Machine {
    /// One entry of `GET /api/machines`. None when it has no id or address.
    pub fn from_json(v: &Value) -> Option<Machine> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        Some(Machine {
            id: text("id")?,
            name: text("name").unwrap_or_default(),
            base_url: text("baseUrl")?.trim_end_matches('/').to_string(),
            addresses: v
                .get("addresses")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Address::from_json).collect())
                .unwrap_or_default(),
            token: v.get("token").and_then(Value::as_str).and_then(Token::new),
            os: text("os"),
            added_at: text("addedAt").unwrap_or_default(),
            ssh_destination: text("sshDestination"),
            tunnel_up: v.get("tunnelUp").and_then(Value::as_bool),
        })
    }

    /// The address this app uses for the machine: `picked` while it is still one of the machine's, else the one home uses
    /// (`baseUrlOf`). Picking an address is piece H's.
    pub fn base_url_with<'a>(&'a self, picked: Option<&'a str>) -> &'a str {
        picked.filter(|p| self.addresses.iter().any(|a| a.url == *p)).unwrap_or(&self.base_url)
    }

    pub fn base(&self) -> &str {
        self.base_url_with(None)
    }

    /// Where this machine's token is held: its own address.
    pub fn hold_token(&self) {
        super::auth::set_for(self.base(), self.token.as_ref().map(Token::expose));
    }

    pub fn target(&self) -> Target {
        Target { key: self.id.clone(), base: self.base().to_string() }
    }
}

/// Where an action on a session goes (`MachineTarget`): home is `home`, another machine is its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub key: String,
    pub base: String,
}

impl Target {
    pub fn home(base: &str) -> Target {
        Target { key: super::settings::HOME_MACHINE_KEY.to_string(), base: base.to_string() }
    }

    pub fn is_home(&self) -> bool {
        self.key == super::settings::HOME_MACHINE_KEY
    }
}

// ─── Kinds and order ─────────────────────────────────────────────────────────

fn ipv4(host: &str) -> Option<[u8; 4]> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let mut out = [0u8; 4];
    for (o, p) in out.iter_mut().zip(&parts) {
        *o = p.parse().ok()?;
    }
    Some(out)
}

fn is_private_ipv4([a, b, ..]: [u8; 4]) -> bool {
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

/// What kind of address `url` is, from its host alone (`kindOf`).
pub fn kind_of(url: &str) -> Kind {
    let Some((_, host, _, _)) = split_url(url) else { return Kind::Other };
    if let Some(v6) = host.strip_prefix('[') {
        let v6 = v6.trim_end_matches(']');
        if v6.starts_with("fd7a:115c:a1e0:") {
            return Kind::TailnetIp;
        }
        let b = v6.as_bytes();
        return if b.len() > 4
            && b[0] == b'f'
            && matches!(b[1], b'c' | b'd')
            && b[2].is_ascii_hexdigit()
            && b[3].is_ascii_hexdigit()
            && b[4] == b':'
        {
            Kind::Lan
        } else {
            Kind::Other
        };
    }
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') && host.matches('.').count() == 3 {
        return match ipv4(&host) {
            Some([100, b, ..]) if (64..=127).contains(&b) => Kind::TailnetIp,
            Some(ip) if is_private_ipv4(ip) => Kind::Lan,
            _ => Kind::Other,
        };
    }
    if host.ends_with(".ts.net") {
        Kind::Tailnet
    } else if (!host.contains('.') && host != "localhost")
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host.ends_with(".home.arpa")
    {
        Kind::Hostname
    } else {
        Kind::Other
    }
}

/// The list with the item at `from` moved to `to` (`moveAddress`): `to` is clamped, a `from` outside the list changes nothing.
pub fn move_address<T: Clone>(list: &[T], from: isize, to: isize) -> Vec<T> {
    let mut next = list.to_vec();
    if from < 0 || from as usize >= next.len() {
        return next;
    }
    let moved = next.remove(from as usize);
    next.insert(to.clamp(0, next.len() as isize) as usize, moved);
    next
}

// ─── Addresses ───────────────────────────────────────────────────────────────

/// `(scheme, host, port, path)` of an `http(s)://` address, host lower case, port only when it isn't the scheme's default,
/// path without trailing slashes. None for anything `new URL` would refuse.
fn split_url(url: &str) -> Option<(String, String, Option<u16>, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (host, port) = match authority.rfind(':') {
        Some(i) if !authority[i..].contains(']') => (&authority[..i], Some(&authority[i + 1..])),
        _ => (authority, None),
    };
    let host = host.to_ascii_lowercase();
    let bracketed = host.starts_with('[') && host.ends_with(']') && host.len() > 2;
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c.is_control() || (!bracketed && "[]<>^|\\%".contains(c))) {
        return None;
    }
    let port = match port {
        None | Some("") => None,
        Some(p) if p.bytes().all(|b| b.is_ascii_digit()) => Some(p.parse::<u16>().ok()?),
        Some(_) => return None,
    };
    let default = if scheme == "https" { 443 } else { 80 };
    let port = port.filter(|p| *p != default);
    let path = tail.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').to_string();
    Some((scheme, host, port, path))
}

/// What a machine's address is typed as (`normalizeBaseUrl`): `http://` added when there's no scheme; scheme, host, port and
/// path kept; trailing slashes dropped.
pub fn normalize_base_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    let full =
        if lower.starts_with("http://") || lower.starts_with("https://") { trimmed.to_string() } else { format!("http://{trimmed}") };
    match split_url(&full) {
        Some((scheme, host, port, path)) => Ok(format!("{scheme}://{host}{}{path}", port.map(|p| format!(":{p}")).unwrap_or_default())),
        None => Err("That isn't an address. Try something like http://100.64.90.72:2113.".to_string()),
    }
}

/// How `base_url` is reached, in a few words (`describeReach`).
pub fn describe_reach(base_url: &str) -> String {
    let Some((_, host, port, _)) = split_url(base_url) else { return format!("at {base_url}") };
    let shown = port.map_or(host.clone(), |p| format!("{host}:{p}"));
    let ip = ipv4(&host);
    if host.ends_with(".ts.net") || matches!(ip, Some([100, b, ..]) if (64..=127).contains(&b)) {
        format!("over Tailscale at {shown}")
    } else if host.ends_with(".local") {
        format!("on your network at {shown}")
    } else if is_loopback_host(&host) {
        format!("on this machine at {shown}")
    } else if ip.is_some_and(is_private_ipv4) {
        format!("on your network at {shown}")
    } else {
        format!("at {shown}")
    }
}

/// Whether `host` (no port) only means this computer (`isLoopbackHost`).
pub fn is_loopback_host(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    bare == "localhost" || bare.ends_with(".localhost") || bare == "::1" || bare.starts_with("127.")
}

// ─── Pairing ─────────────────────────────────────────────────────────────────

/// Pulls a pairing code out of what was typed; `XXXX-XXXX` or none (`normalizeManualCode`).
/// The alphabet has no I, L, O or U; I and L are read as 1 and O as 0.
pub fn normalize_manual_code(typed: &str) -> Option<String> {
    let compact: String = typed
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        })
        .collect();
    let valid = compact.chars().count() == 8
        && compact.chars().all(|c| c.is_ascii_digit() || ("ABCDEFGHJKMNPQRSTV".contains(c) || "WXYZ".contains(c)));
    valid.then(|| format!("{}-{}", &compact[..4], &compact[4..]))
}

/// What was typed or pasted into the Pairing link field (`PairingInput`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairingInput {
    /// A whole link: the address and the code are both known.
    Link { base_url: String, code: String },
    /// Only an address: the code still has to be typed.
    Address { base_url: String },
    /// Looks like a link but isn't one Fleet can use; the message says what's wrong.
    Invalid(String),
}

/// Reads the Pairing link field: `<address>/add-machine#<CODE>`. None while it's empty.
pub fn parse_pairing_input(typed: &str) -> Option<PairingInput> {
    let text = typed.trim();
    if text.is_empty() {
        return None;
    }
    let (address, fragment) = match text.find('#') {
        Some(i) => (&text[..i], Some(&text[i + 1..])),
        None => (text, None),
    };
    let without_slash = address.strip_suffix('/').unwrap_or(address);
    let address = without_slash.strip_suffix("/add-machine").unwrap_or(address);
    let Ok(base_url) = normalize_base_url(address) else {
        return Some(PairingInput::Invalid(
            "That isn't a pairing link or an address. A link looks like http://100.64.90.72:2113/add-machine#ABCD-EFGH.".to_string(),
        ));
    };
    let Some(fragment) = fragment else { return Some(PairingInput::Address { base_url }) };
    Some(match normalize_manual_code(fragment) {
        Some(code) => PairingInput::Link { base_url, code },
        None => PairingInput::Invalid("That link has no valid code. Run fleet node pair on the other machine for a new one.".to_string()),
    })
}
