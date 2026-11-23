//! What Rune draws over the window that a native web view would hide or be drawn over (component 14, piece D, rows 14.4 and
//! 14.8). The overlay layer (`ui::overlays`) writes it while it builds, so every overlay it mounts is counted in one place.
use std::cell::{Cell, RefCell};

/// A floating thing's element key and where it is (none yet: not measured).
type Float = (String, Option<[f32; 4]>);

/// Menus, popovers, selects, the palette, dialogs and the lightbox dim or reach over a page whole; the floating things (a toast,
/// the notice card, the autocomplete popup, the history list, the rail's tooltip) cover only the rectangle they are in.
#[derive(Debug, Default)]
pub struct Cover {
    whole: Cell<bool>,
    /// The open thing (`ui.open`, as `Debug` says it) is a menu whose panels are in `floats`, so it hides the page only where they
    /// are (row 2.88). Stale for any other `ui.open`: a menu opened this frame hides the page whole until its panels are built.
    menu_only: RefCell<Option<String>>,
    /// How many builds in a row have shown the same menu: its panels are placed from the second on.
    menu_builds: Cell<u32>,
    /// The floating things now mounted, by element key, with where each one is (none yet: not measured).
    floats: RefCell<Vec<Float>>,
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

impl Cover {
    /// From the overlay layer's build: `floats` are the keys of what floats now, `whole` that something covers the window.
    /// Where each float is comes from the layout, which the view reads back (it builds again when one moves).
    pub fn set(&self, floats: &[String], whole: bool, menu_only: Option<String>) {
        self.whole.set(whole);
        let again = menu_only.is_some() && *self.menu_only.borrow() == menu_only;
        self.menu_builds.set(if again { self.menu_builds.get() + 1 } else { 0 });
        *self.menu_only.borrow_mut() = menu_only;
        *self.floats.borrow_mut() = floats.iter().map(|k| (k.clone(), rune::memo::window_layout(k))).collect();
    }

    /// `open` is the menu whose panels are among the floats, and all there is over the window.
    pub fn menu_only(&self, open: &str) -> bool {
        self.menu_builds.get() >= 1 && self.menu_only.borrow().as_deref() == Some(open)
    }

    /// Something covers the whole window.
    pub fn whole(&self) -> bool {
        self.whole.get()
    }

    /// A floating thing is over `rect`, or is not placed yet (so it might be).
    pub fn over(&self, rect: [f32; 4]) -> bool {
        self.floats.borrow().iter().any(|(_, at)| at.is_none_or(|at| overlap(at, rect)))
    }

    /// Set by hand, for tests of the rule.
    pub fn put(&self, floats: &[(&'static str, [f32; 4])], whole: bool) {
        self.whole.set(whole);
        *self.floats.borrow_mut() = floats.iter().map(|(k, r)| ((*k).to_string(), Some(*r))).collect();
    }
}
