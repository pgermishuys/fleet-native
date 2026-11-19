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
