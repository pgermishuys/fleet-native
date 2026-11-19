//! Scenes of Settings → API keys (`settings-api-keys-*`): each state the page can be in, drawn with no server. The keys, labels and
//! the one typed value are invented; the value draws as bullets, as it does in the app.
use crate::State;
use crate::fleet::settings::credentials::{self as c, CredentialsPage};
use rune::TextEdit;

#[derive(Clone, Copy)]
pub enum Which {
    Loading,
    Empty,
    List,
    /// The add form with the Anthropic chip clicked and nothing typed for the secret: "All fields are required."
    Form,
    /// The first key's inline edit with a value typed (bullets) and the other key beside it.
    Edit,
    /// A load that failed.
    Failed,
}

pub fn scene(s: &mut State, dark: bool, which: Which) {
    s.dark = Some(dark);
    s.open_settings("credentials", None);
    // Nothing asks a server in a scene: the page has read already.
    let mut page = CredentialsPage { loaded_for: Some(s.settings.choice.key(&s.machines).to_string()), ..Default::default() };
    if !matches!(which, Which::Empty | Which::Form | Which::Loading | Which::Failed) {
        page.keys = crate::mock::credentials::keys();
    }
    match which {
        Which::Loading => page.loading = true,
        Which::Failed => page.load_error = Some("Failed to load API keys.".into()),
        Which::Form => {
            page.form_open = true;
            page.form.apply_preset(&c::PRESETS[0]);
            page.form.error = Some(c::ALL_REQUIRED.into());
        }
        Which::Edit => {
            page.begin_edit("cred-1");
            if let Some(e) = page.editing.as_mut() {
                e.value = TextEdit::new("test-0002".to_string()).secure();
            }
        }
        Which::Empty | Which::List => {}
    }
    s.settings.credentials = page;
}
