//! Scenes of GitHub's connection page (`settings-github-*`): each state the page can be in, drawn with no server. Every name and code
//! in them is invented.
use crate::State;
use crate::fleet::settings::github_auth::{Device, Pat};

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

pub fn scene(s: &mut State, dark: bool, which: Which) {
    s.dark = Some(dark);
    s.open_settings("plugin:github", None);
    s.plugins.statuses = vec![("github".into(), if matches!(which, Which::Connected) { "connected" } else { "disconnected" }.into())];
    let clock = s.clock;
    let g = &mut s.settings.github;
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
