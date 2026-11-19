//! `InstallTargetPicker.vue`: "Install to", a select of Global and every repository, and under it who sees the install and
//! where it goes. Catalog and Custom, of Skills and of Tools, use it, so a piece that draws them calls [`picker`] and mounts
//! [`list`] from `ui::overlays` under its own name.
//! Rune's select has no group labels, so the "Repository" label over the repositories is not drawn (docs/RUNE.md).
use crate::State;
use crate::fleet::settings::install_target::{InstallTarget, Kind};
use crate::theme::{Theme, sans};
use crate::ui::{Open, rem, select};
use rune::{El, Span, col, rich, row, text};
use std::rc::Rc;

/// What the picker reads: the choice, whether an install is under way, and the repositories; Skills and Tools each keep their own.
fn parts(s: &State, kind: Kind) -> (&InstallTarget, bool, &[(String, String)]) {
    match kind {
        Kind::Tools => (&s.settings.tools.target, s.settings.tools.installing, &s.settings.tools.repositories),
        _ => (&s.settings.skills.target, s.settings.skills.installing, &s.settings.skills.repositories),
    }
}

/// The label, the select (disabled while `busy`), and the line under it. `name` keys it and its open list.
pub fn picker(s: &State, t: &Theme, name: &'static str, kind: Kind) -> El {
    let (target, installing, _) = parts(s, kind);
    let label = text("INSTALL TO").font(sans(rem(0.75), 500.0, 1.33).tracking(0.525)).color(t.muted);
    let trigger = select::trigger(t, &target.label(), false, false, 0.0).w_full().key(name.to_string());
    let trigger = if installing {
        crate::ui::button::disabled(trigger)
    } else {
        trigger.on_press(move |s: &mut State| s.ui.open = Some(Open::Select(name)))
    };
    let destination = target.destination(kind);
    let words = format!("{} \u{b7} {destination}", target.audience());
    let at = target.audience().len() + " \u{b7} ".len();
    let line =
        rich(words.clone(), vec![Span::new(at, words.len()).mono()]).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full();
    col().key(format!("{name}-picker")).w_full().gap(3.5).child(label).child(trigger).child(row_line(line, name))
}

fn row_line(line: El, name: &str) -> El {
    row().key(format!("{name}-line")).w_full().child(line)
}

/// The open list: Global, then every repository by name; none are listed while they are still being read.
pub fn list(s: &State, t: &Theme, name: &'static str) -> Vec<El> {
    let kind = if name == "tools-target" { Kind::Tools } else { Kind::Skills };
    let (target, _, repositories) = parts(s, kind);
    let global = InstallTarget::Global.key();
    let mut keys = vec![global.clone()];
    let mut labels = vec!["Global".to_string()];
    for (path, repo) in repositories {
        keys.push(InstallTarget::Project(path.clone()).key());
        labels.push(repo.clone());
    }
    let options: Vec<(&str, &str)> = keys.iter().zip(&labels).map(|(k, l)| (k.as_str(), l.as_str())).collect();
    let choose: select::Choose = if kind == Kind::Tools {
        Rc::new(|s: &mut State, key: &str| s.choose_tools_target(key))
    } else {
        Rc::new(|s: &mut State, key: &str| s.choose_skills_target(key))
    };
    select::content(t, &format!("{name}-list"), name, 280.0, &options, Some(&target.key()), choose)
}
