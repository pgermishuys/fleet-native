//! The sign-in page's scenes (component 9, piece S): the token form in each state Fleet's page can be in. The token a scene types is
//! made up and opens nothing.
use crate::State;
use crate::fleet::sign_in::{INVALID, Options};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// `GET /api/auth/status` has not answered.
    Checking,
    Empty,
    /// A made-up token typed, with the field showing bullets.
    Typed,
    Signing,
    /// A made-up token the Fleet turned away.
    Invalid,
    /// A cloud Fleet.
    Browser,
}

pub const SCENE_TOKEN: &str = "invented-token-0000";

impl State {
    pub fn show_sign_in(&mut self, dark: bool, case: Case) {
        self.dark = Some(dark);
        self.sign_in.open = true;
        self.sign_in.options = match case {
            Case::Checking => Options::Loading,
            Case::Browser => Options::Oidc,
            _ => Options::Token,
        };
        match case {
            Case::Empty => self.sign_in.token.focused = true,
            Case::Typed | Case::Signing | Case::Invalid => self.sign_in.token.replace_text(SCENE_TOKEN),
            Case::Checking | Case::Browser => {}
        }
        self.sign_in.submitting = case == Case::Signing;
        self.sign_in.error = (case == Case::Invalid).then(|| INVALID.to_string());
    }
}
