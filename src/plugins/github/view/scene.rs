//! Scenes of GitHub's connection page (`settings-github-*`): each state the page can be in, drawn with no server. Every name and code
//! in them is invented.
use crate::State;
use crate::plugins::github::auth::{Device, Pat};

#[derive(Clone, Copy)]
pub enum Which {
    Checking,
    Connected,
    Idle,
    Waiting,
    Denied,
    /// The token card open, with its message: the owner of a token that tested well.
    TokenTested,
    /// The token card open, with a refusal.
    TokenRefused,
}

/// The scene named `which`: `checking`, `connected`, `idle`, `waiting`, `denied`, `tokentested` or `tokenrefused`.
pub fn set(s: &mut State, which: &str, dark: bool) {
    let which = match which {
        "checking" => Which::Checking,
        "connected" => Which::Connected,
        "idle" => Which::Idle,
        "waiting" => Which::Waiting,
        "denied" => Which::Denied,
        "tokentested" => Which::TokenTested,
        "tokenrefused" => Which::TokenRefused,
        other => panic!("no GitHub scene called {other}"),
    };
    scene(s, dark, which)
}

pub fn scene(s: &mut State, dark: bool, which: Which) {
    s.dark = Some(dark);
    s.open_settings("plugin:github", None);
    s.plugins.statuses = vec![("github".into(), if matches!(which, Which::Connected) { "connected" } else { "disconnected" }.into())];
    let clock = s.clock;
    let g = crate::plugins::github::auth(&mut s.places);
    // Nothing asks a server in a scene: the page has read already.
    g.asked = true;
    g.checking = matches!(which, Which::Checking);
    g.connected = matches!(which, Which::Connected);
    match which {
        Which::Waiting => {
            g.device = Device::Awaiting {
                user_code: "WXYZ-1234".into(),
                verification_uri: "https://github.test/login/device".into(),
                device_code: "dev_invented".into(),
                expires_at: clock + 899.5,
                interval: 5,
            }
        }
        Which::Denied => g.device = Device::Denied,
        Which::TokenTested | Which::TokenRefused => {
            g.pat_open = true;
            g.pat.replace_text("ghp_invented_0000");
            g.token_retyped();
            g.pat_state = if matches!(which, Which::TokenTested) {
                Pat::Success(Some("octo-test".into()))
            } else {
                Pat::Error("Bad credentials".into())
            };
        }
        _ => {}
    }
}
