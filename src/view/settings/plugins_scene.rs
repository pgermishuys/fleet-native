//! Scenes of Settings → Plugins (`settings-plugins*`, `settings-plugin-*`): the overview and a plugin's page, with a plugin on or off.
use crate::State;

/// What a scene sets: the page, the plugins that are off, whether Mods is on, and the connection status the server reports.
pub struct Look {
    pub dark: bool,
    pub page: &'static str,
    pub off: &'static [&'static str],
    pub mods_on: bool,
    pub status: Option<&'static str>,
    pub none_installed: bool,
}

pub fn scene(s: &mut State, look: Look) {
    s.dark = Some(look.dark);
    let list: Vec<String> = look.off.iter().map(|id| format!("{id:?}")).collect();
    s.pickers.prefs.set("plugins.disabled", &format!("[{}]", list.join(",")));
    s.pickers.prefs.set("Mods", if look.mods_on { "true" } else { "false" });
    if let Some(status) = look.status {
        s.plugins.statuses = vec![("github".into(), status.into())];
    }
    s.settings.plugins_none = look.none_installed;
    s.open_settings(look.page, None);
}

/// Which state of the Mods panel a `settings-mods-*` scene shows.
#[derive(Clone, Copy)]
pub enum ModsLook {
    Confirm,
    ConfirmFound,
    Progress,
    Failed,
    Ready,
}

/// The Plugins page (or Mods' own page, for Ready) with the Mods panel in one of its states, from an invented view of the Bun
/// runtime. `loaded_for` is set so the page doesn't read a machine that isn't there.
pub fn mods_scene(s: &mut State, dark: bool, look: ModsLook) {
    use crate::fleet::settings::mods_runtime::{Bun, Found, Job, ModsRuntime, Release, View};
    let release = Release {
        version: "1.4.2".into(),
        size: Some(35_300_000),
        has_build: true,
        install_folder: "~/.weave/runtimes/bun/1.4.2".into(),
        source: "github.com/oven-sh/bun".into(),
    };
    let job = |phase: &str| Job {
        phase: phase.into(),
        version: "1.4.2".into(),
        bytes_received: Some(12_000_000),
        bytes_total: Some(35_300_000),
        ..Default::default()
    };
    let mut view = View { release, ..Default::default() };
    let mut run = ModsRuntime::default();
    let mods_on = matches!(look, ModsLook::Ready);
    match look {
        ModsLook::Confirm => run.confirming = true,
        ModsLook::ConfirmFound => {
            view.found =
                vec![Found { path: "/home/dev/.bun/bin/bun".into(), display_path: "~/.bun/bin/bun".into(), version: "1.4.2".into() }];
            run.confirming = true;
        }
        ModsLook::Progress => {
            view.job = Some(job("downloading"));
            run.came_back = true;
        }
        ModsLook::Failed => {
            view.job = Some(Job {
                phase: "failed".into(),
                version: "1.4.2".into(),
                reason: Some("offline".into()),
                message: Some("Fleet couldn't reach github.com: the connection timed out.".into()),
                ..Default::default()
            })
        }
        ModsLook::Ready => {
            view.bun = Some(Bun { display_path: "~/.weave/runtimes/bun/1.4.2/bun".into(), configured: false, version: "1.4.2".into() })
        }
    }
    run.view = Some(view);
    run.loaded_for = Some(s.settings.choice.key(&s.machines).to_string());
    scene(s, Look { dark, page: if mods_on { "plugin:mods" } else { "plugins" }, off: &[], mods_on, status: None, none_installed: false });
    s.settings.mods_runtime = run;
}
