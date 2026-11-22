//! The address of a page a mod ships (`lib/mods/page-url.ts`, component 22 piece J), built in one place from the mod that owns
//! it, the session, the path and the query: `/api/sessions/{session}/mods/{mod id}/pages/{path}` on the session's machine.
//!
//! The path comes from a mod, so it is hostile: each segment is encoded, and anything that could climb out of the mod's folder,
//! name a scheme or leave the machine's API origin is refused (`None`).
use super::types::is_mod_id;
use serde_json::{Map, Value};

/// `MOD_SESSION_ID_PATTERN`: letters, digits, `_` and `-`, 1 to 128 of them, so never `.`, `..` or anything with a slash.
pub fn is_session_id(id: &str) -> bool {
    (1..=128).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `encodeURIComponent` keeps these besides letters and digits.
fn component_keeps(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b)
}

/// `URLSearchParams` (form encoding) keeps these besides letters and digits; a space is `+`.
fn form_keeps(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"*-._".contains(&b)
}

fn encode(s: &str, keeps: fn(u8) -> bool, space_plus: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if keeps(b) {
            out.push(b as char);
        } else if space_plus && b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `pathSegments`: the path's segments, or nothing when it isn't a safe `.html` path.
fn path_segments(path: &str) -> Option<Vec<&str>> {
    let control = |c: char| c <= '\u{1f}' || c == '\u{7f}';
    if !path.ends_with(".html") || path.contains(control) || path.contains('\\') || path.contains(':') {
        return None;
    }
    let segments: Vec<&str> = path.split('/').collect();
    // A leading or trailing slash and `a//b` leave an empty segment.
    segments.iter().all(|s| !matches!(*s, "" | "." | "..")).then_some(segments)
}

/// A URL's origin (`scheme://authority`), or nothing when it has none that can be read. An address with no scheme (a relative
/// one) is on the page's own origin: the empty origin.
fn origin(url: &str) -> Option<&str> {
    let Some((scheme, rest)) = url.split_once("://") else {
        return (!url.contains(':') || url.starts_with('/')).then_some("");
    };
    if scheme.is_empty() || !scheme.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b)) {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    // An IPv6 host is in brackets and must close them; a bracket anywhere else is no host.
    let bracketed = authority.starts_with('[');
    if authority.is_empty() || (authority.contains(['[', ']']) && !(bracketed && authority.contains(']'))) {
        return None;
    }
    Some(&url[..scheme.len() + 3 + end])
}

/// `modPageUrl`: the page's address, or `None` when the mod, session, path or query can't make a safe one. `api_url` makes an
/// address on the machine's API from a path, as Fleet's `apiUrl` does; `query` is an object of which only the strings count.
pub fn mod_page_url(
    api_url: impl Fn(&str) -> String,
    session: &str,
    mod_id: &str,
    path: &str,
    query: Option<&Map<String, Value>>,
) -> Option<String> {
    if !is_session_id(session) || !is_mod_id(mod_id) {
        return None;
    }
    let segments = path_segments(path)?;
    let params: Vec<String> = query
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| Some(format!("{}={}", encode(name, form_keeps, true), encode(value.as_str()?, form_keeps, true))))
        .collect();
    let search = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
    let address = api_url(&format!(
        "/api/sessions/{}/mods/{}/pages/{}{search}",
        encode(session, component_keeps, false),
        encode(mod_id, component_keeps, false),
        segments.iter().map(|s| encode(s, component_keeps, false)).collect::<Vec<_>>().join("/"),
    ));
    (origin(&address)? == origin(&api_url("/"))?).then_some(address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn api(path: &str) -> String {
        format!("http://fleet.test:5000{path}")
    }

    fn url(path: &str, query: Option<Value>) -> Option<String> {
        url_with(api, "ses_1", "demo-mod@v3", path, query)
    }

    fn url_with(api_url: impl Fn(&str) -> String, session: &str, mod_id: &str, path: &str, query: Option<Value>) -> Option<String> {
        mod_page_url(api_url, session, mod_id, path, query.as_ref().and_then(Value::as_object))
    }

    const BASE: &str = "http://fleet.test:5000/api/sessions/ses_1/mods/demo-mod%40v3/pages/";

    #[test]
    fn builds_the_address_m6_serves_on_the_machines_api() {
        assert_eq!(url("ui/report.html", None).unwrap(), format!("{BASE}ui/report.html"));
        assert_eq!(url("a.html", Some(json!({"run": "1", "who": "me"}))).unwrap(), format!("{BASE}a.html?run=1&who=me"));
    }

    #[test]
    fn has_no_query_mark_for_an_empty_or_missing_query() {
        assert_eq!(url("a.html", Some(json!({}))).unwrap(), format!("{BASE}a.html"));
        assert!(!url("a.html", None).unwrap().contains('?'));
    }

    #[test]
    fn encodes_every_segment() {
        let built = url("my ui/r&d é#1?.html", None).unwrap();
        assert_eq!(built, format!("{BASE}my%20ui/r%26d%20%C3%A9%231%3F.html"));
        assert!(!built.contains('#') && !built.contains('?'));
    }

    #[test]
    fn refuses_a_session_id_outside_fleets_charset() {
        for session in [".", "..", "", "a/b", "a.b", "a?c", &"a".repeat(129)] {
            assert_eq!(url_with(api, session, "demo-mod@v3", "a.html", None), None, "{session:?}");
        }
        let ok = |session: &str| url_with(api, session, "demo-mod@v3", "a.html", None);
        assert!(ok("3f2b8c1e-5d4a-4e9b-8a77-0c1d2e3f4a5b").unwrap().contains("/api/sessions/3f2b8c1e-"));
        assert!(ok("ses_ABC-123").unwrap().contains("/api/sessions/ses_ABC-123/mods/"));
        assert!(ok(&"a".repeat(128)).is_some());
    }

    #[test]
    fn builds_the_mod_ids_draft_part_from_the_same_session_id_rule() {
        assert!(!is_session_id("a.b"));
        assert!(is_mod_id("m@draft:ses_ABC-123"));
        assert!(!is_mod_id(&format!("m@draft:{}", "a".repeat(129))));
        assert!(!is_mod_id("m@draft:a.b"));
    }

    #[test]
    fn builds_the_query_with_form_encoding() {
        let built = url("a.html", Some(json!({"a&b": "c=d", "q r": "é/#"}))).unwrap();
        // The keys come in either order: the map may be sorted.
        assert!(built.ends_with("?a%26b=c%3Dd&q+r=%C3%A9%2F%23"), "{built}");
        assert!(!built.contains('#'));
    }

    #[test]
    fn rejects_paths_that_climb_name_a_scheme_or_leave_the_machine() {
        let bad = [
            "../x.html",
            "ui/../../api/x.html",
            "ui/..",
            "./x.html",
            "ui/./x.html",
            "ui\\x.html",
            "/api/sessions/x.html",
            "ui//x.html",
            "ui/x.html/",
            "",
            "javascript:alert(1).html",
            "JaVaScRiPt:alert(1)//.html",
            "data:text/html,x.html",
            "ui/x:y.html",
            "https://evil.example/x.html",
            "//evil.example/x.html",
            "ui/x\n.html",
            "ui/\tx.html",
            "ui/x\u{0}.html",
            "ui/x\u{7f}.html",
            "ui/x.js",
            "ui/x",
            "x.html/y",
            "x.HTML",
        ];
        for path in bad {
            assert_eq!(url(path, None), None, "{path:?}");
        }
    }

    #[test]
    fn rejects_a_bad_mod_id_or_an_empty_session_id() {
        let long = format!("{}@v1", "a".repeat(65));
        for id in [
            "",
            "demo-mod",
            "A@v1",
            "1a@v1",
            "a_b@v1",
            "a/b@v1",
            "..",
            "a b@v1",
            &long,
            "x.html",
            "a@v0",
            "a@draft:",
            "a@v1/../b",
            "a@draft:x/y",
        ] {
            assert_eq!(url_with(api, "ses_1", id, "a.html", None), None, "{id:?}");
        }
        assert_eq!(url_with(api, "", "demo-mod@v3", "a.html", None), None);
    }

    #[test]
    fn takes_a_mod_id_as_one_segment_and_a_drafts_session_in_it() {
        assert!(url_with(api, "ses_1", &format!("a{}@v12", "b".repeat(63)), "a.html", None).is_some());
        let draft = url_with(api, "ses_1", "demo@draft:ses_0-1", "a.html", None).unwrap();
        assert_eq!(draft, "http://fleet.test:5000/api/sessions/ses_1/mods/demo%40draft%3Ases_0-1/pages/a.html");
        let path = draft.splitn(4, '/').nth(3).unwrap();
        assert_eq!(path.split('/').count(), 7, "one segment for the id");
    }

    #[test]
    fn accepts_only_an_address_on_the_machines_api_origin() {
        // The base decides the origin: a path that stays inside it can't leave it.
        let other = |p: &str| format!("https://other.example{p}");
        assert_eq!(
            url_with(other, "ses_1", "demo-mod@v3", "a.html", None).unwrap(),
            "https://other.example/api/sessions/ses_1/mods/demo-mod%40v3/pages/a.html"
        );
        // An api_url whose answer is on another origin than its own root is refused.
        let calls = std::cell::Cell::new(0);
        let drifting = |p: &str| {
            if p == "/" && calls.replace(calls.get() + 1) == 0 {
                "http://fleet.test:5000/".to_string()
            } else {
                format!("http://evil.example{p}")
            }
        };
        assert_eq!(url_with(drifting, "ses_1", "demo-mod@v3", "a.html", None), None);
        // A relative api_url (same origin) works against the page's own origin.
        assert_eq!(
            url_with(|p| p.to_string(), "ses_1", "demo-mod@v3", "a.html", None).unwrap(),
            "/api/sessions/ses_1/mods/demo-mod%40v3/pages/a.html"
        );
        // Something that is not a URL at all.
        assert_eq!(url_with(|_| "http://[bad".into(), "ses_1", "demo-mod@v3", "a.html", None), None);
    }

    #[test]
    fn ignores_query_values_that_are_not_strings() {
        assert!(url("a.html", Some(json!({"a": "1", "b": 2}))).unwrap().ends_with("?a=1"));
    }
}
