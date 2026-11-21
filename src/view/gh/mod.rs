//! The small GitHub parts (`components/github/`): a pull request's state pill, a check's icon, the checks bar, the diff
//! stat, a reviewer's avatar, an item's icon and a label. Plain views with no state of their own. The header's pull request
//! pill and the Context tab draw them, and the GitHub pages (component 21) reuse them.
mod check_icon;
mod checks_summary;
mod diff_stat;
mod gallery;
mod item_icon;
mod label;
mod pr_state_pill;
mod reviewer_avatar;

pub use check_icon::{CheckKind, check_icon};
pub use checks_summary::checks_summary;
pub use diff_stat::diff_stat;
pub use gallery::parts;
pub use item_icon::{ItemKind, item_icon};
pub use label::{label, label_color};
pub use pr_state_pill::pr_state_pill;
pub use reviewer_avatar::reviewer_avatar;
