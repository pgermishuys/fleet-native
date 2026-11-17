//! The Install panel of Settings → Harnesses (`HarnessInstallPanel.vue`) as plain rules: whether a harness needs installing,
//! the places it can go, the folders, the commands to copy. Shown for a harness that describes its install (`setup.mode`),
//! such as OpenCode 2. No Rune and no I/O here.
use super::super::harness_setup_rows::{Choice, Folder};
use super::super::new_session::harness_choice::HarnessEntry;

/// What the panel draws for one harness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    /// `setup.mode`, the chip by the title; hidden when two places are offered.
    pub mode: Option<String>,
    /// "Not installed yet", "Needs installing again", or the version; none when there is nothing to say.
    pub version: Option<String>,
    /// More than one place and it needs installing: each is a card of its own.
    pub choices: Vec<Choice>,
    /// The harness's own folders, when there is no choice.
    pub folders: Vec<Folder>,
    pub notes: Vec<String>,
    /// The one installer, when it needs installing and there is no place to pick.
    pub install_command: Option<String>,
    /// The button "Install {name} here": the machine is live, it needs installing, and there is something to run.
    pub set_up: bool,
    /// The sign-in command: only once it is installed and working.
    pub sign_in: Option<String>,
    /// "Or sign in to a provider in a terminal:" when Fleet signs in itself, else "Sign in to a provider in a terminal:".
    pub sign_in_lead: &'static str,
    /// "Download {name}": the page to fetch it from by hand, until it is installed.
    pub download: Option<String>,
}

/// Not installed, or what is there isn't this harness any more: it is installed (again) with the installer (`needsInstall`).
pub fn needs_install(h: &HarnessEntry) -> bool {
    matches!(h.setup_state(), "not-installed" | "not-working")
}

fn installed(h: &HarnessEntry) -> bool {
    h.executable_path.as_deref().is_some_and(|p| !p.is_empty())
}

/// `None` unless the harness describes its install. `live` is whether the machine shown is the one the app runs on (Install
/// here acts on that one only); `signs_in_here` is whether Fleet lists the harness's providers itself.
pub fn panel(h: &HarnessEntry, live: bool, signs_in_here: bool) -> Option<Panel> {
    let setup = &h.setup;
    let mode = setup.mode.clone()?;
    let needs = needs_install(h);
    let choices: Vec<Choice> = if needs && setup.install_choices.len() > 1 { setup.install_choices.clone() } else { Vec::new() };
    let has_choices = !choices.is_empty();
    let install_command = (needs && !has_choices).then(|| setup.install_command.clone()).flatten();
    let version = if needs {
        Some(if installed(h) { "Needs installing again" } else { "Not installed yet" }.to_string())
    } else {
        h.version.clone().filter(|v| !v.is_empty())
    };
    Some(Panel {
        mode: (!has_choices).then_some(mode),
        version,
        folders: if has_choices { Vec::new() } else { setup.folders.clone() },
        notes: setup.notes.clone(),
        set_up: live && needs && (install_command.is_some() || has_choices),
        sign_in: (installed(h) && !needs).then(|| setup.sign_in_command.clone()).flatten(),
        sign_in_lead: if signs_in_here { "Or sign in to a provider in a terminal:" } else { "Sign in to a provider in a terminal:" },
        download: setup.download_url.clone().filter(|_| !installed(h)),
        install_command,
        choices,
    })
}

/// Fleet lists the harness's providers itself (`offersSignIn`): it signs in with a key or in the browser, and the terminal is the
/// fallback.
pub fn offers_sign_in(h: &HarnessEntry) -> bool {
    h.card.supports_provider_sign_in && (h.available || h.state == "sign-in-required")
}
