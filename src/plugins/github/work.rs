//! Your GitHub work (`composables/use-github-work.ts`): what needs you across the repositories you follow, one copy shared by the
//! panel's counts and Your work, kept 60 seconds, one request in flight, `refresh` forcing a new one. The request goes on a
//! thread and its answer comes back to the UI thread; nothing here waits.
use super::items::{Work, error_words};
use crate::State;
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;

/// `STALE_AFTER_MS`, in the app's clock seconds.
pub const STALE_AFTER: f64 = 60.0;
const FAILED: &str = "Couldn't load your GitHub work.";

#[derive(Debug, Default)]
pub struct WorkStore {
    pub work: Option<Work>,
    pub loading: bool,
    pub error: Option<String>,
    /// The app's clock when the copy on screen arrived.
    pub loaded_at: Option<f64>,
}

impl WorkStore {
    fn fresh(&self, now: f64) -> bool {
        self.work.is_some() && self.loaded_at.is_some_and(|at| now - at < STALE_AFTER)
    }
}

impl State {
    /// `load`: the shared copy, fetched again only when it is older than a minute or there is none.
    pub fn github_work_load(&mut self) {
        self.github_work_fetch(false);
    }

    /// `refresh`: a new copy whatever the age.
    pub fn github_work_refresh(&mut self) {
        self.github_work_fetch(true);
    }

    fn github_work_fetch(&mut self, force: bool) {
        let now = self.clock;
        let w = &mut super::state(&mut self.places).work;
        if (!force && w.fresh(now)) || w.loading {
            return;
        }
        w.loading = true;
        w.error = None;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("GET", base, "/api/integrations/github/work", None),
            |s, _, answer| {
                let clock = s.clock;
                let w = &mut super::state(&mut s.places).work;
                w.loading = false;
                match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        w.work = Some(Work::read(&serde_json::from_str(&body).unwrap_or_default()));
                        w.loaded_at = Some(clock);
                    }
                    Ok((status, body)) => w.error = Some(error_words(status, &body)),
                    Err(_) => w.error = Some(FAILED.into()),
                }
            },
        );
        if !sent {
            let w = &mut super::state(&mut self.places).work;
            w.loading = false;
            w.error = Some(FAILED.into());
        }
    }
}
