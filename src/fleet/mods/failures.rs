//! A tree Fleet refused to draw, told to whoever listens (port of `lib/mods/failures.ts`). Fleet's own counts these
//! against the mods that made the tree. Unlike the TypeScript's module-level set, a `Failures` is a value the app owns:
//! tests run on threads side by side and must not hear each other's trees.
use super::types::{Author, Site};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub site: Site,
    pub reason: String,
    pub mods: Vec<Author>,
}

type Listener = Arc<dyn Fn(&Failure) + Send + Sync>;
type Warn = Arc<dyn Fn(&str) + Send + Sync>;

/// Identifies one listener, to stop it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListenerId(u64);

struct Inner {
    next: u64,
    listeners: Vec<(u64, Listener)>,
    warn: Warn,
}

/// The listeners for invalid trees. Clones share them.
#[derive(Clone)]
pub struct Failures(Arc<Mutex<Inner>>);

impl Default for Failures {
    fn default() -> Self {
        // Fleet warns in development only; a release build says nothing.
        let warn: Warn = Arc::new(|message| {
            if cfg!(debug_assertions) {
                eprintln!("{message}");
            }
        });
        Failures(Arc::new(Mutex::new(Inner { next: 0, listeners: Vec::new(), warn })))
    }
}

impl Failures {
    /// Calls `listener` for each invalid tree until `off`.
    pub fn on_invalid(&self, listener: impl Fn(&Failure) + Send + Sync + 'static) -> ListenerId {
        let mut inner = self.0.lock().unwrap();
        inner.next += 1;
        let id = inner.next;
        inner.listeners.push((id, Arc::new(listener)));
        ListenerId(id)
    }

    pub fn off(&self, id: ListenerId) {
        self.0.lock().unwrap().listeners.retain(|(i, _)| *i != id.0);
    }

    /// Where development warnings go (the default is stderr in a debug build).
    pub fn set_warn(&self, warn: impl Fn(&str) + Send + Sync + 'static) {
        self.0.lock().unwrap().warn = Arc::new(warn);
    }

    pub fn report(&self, failure: &Failure) {
        let (listeners, warn): (Vec<Listener>, Warn) = {
            let inner = self.0.lock().unwrap();
            (inner.listeners.iter().map(|(_, l)| l.clone()).collect(), inner.warn.clone())
        };
        let names: Vec<&str> = failure.mods.iter().map(|m| m.name.as_str()).collect();
        warn(&format!("[mods] {} tree from {} is invalid: {}", failure.site.name(), names.join(", "), failure.reason));
        for listener in listeners {
            // A listener that panics doesn't stop the others.
            if catch_unwind(AssertUnwindSafe(|| listener(failure))).is_err() {
                warn("[mods] a failure listener threw");
            }
        }
    }
}
