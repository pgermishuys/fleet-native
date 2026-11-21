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
    /// Connected, the repository cache loaded four minutes ago.
    Cached,
    /// Connected, the cache being read for the first time.
    CacheLoading,
    /// Connected, the cache refused with nothing cached.
    CacheFailed,
    /// Connected, repositories cached and the last refresh refused.
    CacheStale,
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
        "cached" => Which::Cached,
        "cacheloading" => Which::CacheLoading,
        "cachefailed" => Which::CacheFailed,
        "cachestale" => Which::CacheStale,
        other => panic!("no GitHub scene called {other}"),
    };
    scene(s, dark, which)
}

pub fn scene(s: &mut State, dark: bool, which: Which) {
    s.dark = Some(dark);
    s.open_settings("plugin:github", None);
    s.plugins.statuses = vec![("github".into(), if matches!(which, Which::Connected | Which::Cached | Which::CacheLoading | Which::CacheFailed | Which::CacheStale) { "connected" } else { "disconnected" }.into())];
    let clock = s.clock;
    let g = crate::plugins::github::auth(&mut s.places);
    // Nothing asks a server in a scene: the page has read already.
    g.asked = true;
    g.checking = matches!(which, Which::Checking);
    g.connected = matches!(which, Which::Connected | Which::Cached | Which::CacheLoading | Which::CacheFailed | Which::CacheStale);
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
    fill_cache(s, which);
}

/// The repository cache of a connected page: invented repositories, read at a fixed time so "updated 4m ago" never moves.
fn fill_cache(s: &mut State, which: Which) {
    use crate::plugins::github::repos::Repo;
    let now = 20_000 * 86_400_000 + 12 * 3_600_000;
    s.now_ms = Some(now);
    let mut c = crate::plugins::github::state(&mut s.places).repos.get();
    let repo = |i: u64, full: &str| Repo {
        id: i.to_string(),
        full_name: full.into(),
        name: full.rsplit('/').next().unwrap_or_default().into(),
        owner_login: full.split('/').next().unwrap_or_default().into(),
        private: false,
        language: Some("Rust".into()),
        stars: i,
    };
    match which {
        Which::Cached | Which::CacheStale => {
            c.repos = ["acme/rocket", "acme/anvil", "acme/billing", "lumen-labs/atlas", "lumen-labs/beacon", "lumen-labs/compass"]
                .iter()
                .enumerate()
                .map(|(i, n)| repo(i as u64, n))
                .collect();
            c.updated_ms = Some(now - 4 * 60_000);
            if matches!(which, Which::CacheStale) {
                c.error = Some("GitHub didn't answer (HTTP 502).".into());
            }
        }
        Which::CacheLoading => c.loading = true,
        Which::CacheFailed => c.error = Some("GitHub rate limit exceeded. Try again in a few minutes.".into()),
        _ => {}
    }
}
