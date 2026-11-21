//! What a site asks: draw this, or draw Fleet's own (port of `lib/mods/resolve.ts`). A tree is checked once per site
//! and reported once; an invalid one draws as if no mod had contributed.
use super::failures::{Failure, Failures};
use super::points::{HasTree, ModTreeView, is_draft_view};
use super::types::Site;
use super::validate::{Check, validate_tree};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

/// What a site draws.
pub enum Resolved<V> {
    /// No mod draws here: Fleet's own. (One shape for every such answer, so a reader that gets it twice sees no change.)
    NothingDrawn,
    Draws {
        /// The tree to draw: the view's own `Arc` when nothing was cut, else the cut copy.
        tree: Arc<Value>,
        draft: bool,
        view: Arc<V>,
    },
}

impl<V> Resolved<V> {
    pub fn draws(&self) -> bool {
        matches!(self, Resolved::Draws { .. })
    }
}

#[derive(Clone)]
enum Checked {
    Ok(Arc<Value>),
    Invalid,
}

struct Known {
    tree: Weak<Value>,
    site: Site,
    result: Checked,
}

/// Checks each tree once per site. Holds the results by tree identity (a `Weak`, so a tree that is gone is forgotten).
pub struct Resolver {
    failures: Failures,
    known: Mutex<HashMap<usize, Vec<Known>>>,
    validations: AtomicUsize,
}

impl Resolver {
    pub fn new(failures: Failures) -> Self {
        Resolver { failures, known: Mutex::new(HashMap::new()), validations: AtomicUsize::new(0) }
    }

    pub fn failures(&self) -> &Failures {
        &self.failures
    }

    /// How many times a tree has been validated (for tests that say "once per site").
    pub fn validations(&self) -> usize {
        self.validations.load(Ordering::Relaxed)
    }

    fn validate(&self, view: &ModTreeView, site: Site) -> Checked {
        self.validations.fetch_add(1, Ordering::Relaxed);
        match validate_tree(&view.tree, site) {
            Check::Ok { tree: std::borrow::Cow::Borrowed(_), .. } => Checked::Ok(view.tree.clone()),
            Check::Ok { tree, .. } => Checked::Ok(Arc::new(tree.into_owned())),
            Check::Invalid { reason } => {
                self.failures.report(&Failure { site, reason, mods: view.mods.clone() });
                Checked::Invalid
            }
        }
    }

    fn check(&self, view: &ModTreeView, site: Site) -> Checked {
        // Only a tree object or array is remembered. `null` is valid and cheap; a bare string or number is invalid and
        // checked each time, and never reported (as in Fleet).
        if !matches!(*view.tree, Value::Object(_) | Value::Array(_)) {
            self.validations.fetch_add(1, Ordering::Relaxed);
            return match validate_tree(&view.tree, site) {
                Check::Ok { .. } => Checked::Ok(view.tree.clone()),
                Check::Invalid { .. } => Checked::Invalid,
            };
        }
        let key = Arc::as_ptr(&view.tree) as usize;
        {
            let known = self.known.lock().unwrap();
            let found = known
                .get(&key)
                .and_then(|v| v.iter().find(|k| k.site == site && k.tree.upgrade().is_some_and(|t| Arc::ptr_eq(&t, &view.tree))));
            if let Some(found) = found {
                return found.result.clone();
            }
        }
        let result = self.validate(view, site);
        let mut known = self.known.lock().unwrap();
        // A freed tree's address can be reused, so entries whose tree is gone go first.
        known.retain(|_, list| {
            list.retain(|k| k.tree.strong_count() > 0);
            !list.is_empty()
        });
        known.entry(key).or_default().push(Known { tree: Arc::downgrade(&view.tree), site, result: result.clone() });
        result
    }

    /// Checks a site's view once per tree. An invalid tree is reported and draws as if no mod had contributed.
    pub fn resolve<V: HasTree>(&self, view: Option<&Arc<V>>, site: Site) -> Resolved<V> {
        let Some(view) = view else { return Resolved::NothingDrawn };
        let base = view.base();
        if base.mods.is_empty() {
            return Resolved::NothingDrawn;
        }
        match self.check(base, site) {
            Checked::Invalid => Resolved::NothingDrawn,
            Checked::Ok(tree) => Resolved::Draws { tree, draft: is_draft_view(base), view: view.clone() },
        }
    }
}
