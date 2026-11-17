//! The sign-in page (`components/auth/LoginPage.vue`): what a Fleet that turned the app away for want of a token is asked for.
//! Fleet's page lives in the browser and ends in a cookie; here the typed token is checked the same way (`POST /auth/token-login`),
//! then held for this run as the home address's token (`fleet::auth`) and the connection is started again. It is never written
//! to disk, never logged, and never part of anything `Debug` prints.
use super::auth;
use super::http;
use super::link::{Connection, Link};
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};
use std::fmt;

/// The key of the token field.
pub const TOKEN_FIELD: &str = "sign-in-token";

/// What a refused sign-in says, for a blank token and for any failure alike (`submitToken`).
pub const INVALID: &str = "Invalid token";

/// `authMode` of `LoginPage.vue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Options {
    /// `GET /api/auth/status` has not answered.
    #[default]
    Loading,
    Token,
    /// A cloud Fleet signs in through the browser; the app has no way to (row 11.5).
    Oidc,
    /// The status could not be read: Fleet still offers the token form.
    Unknown,
}

impl Options {
    /// `loadAuthMode`: `authEnabled` is browser sign-in, `tokenAuthEnabled` the token form; a status that says neither leaves the form,
    /// because the Fleet did refuse.
    pub fn from_status(status: &Value) -> Options {
        let on = |name: &str| status.get(name).and_then(Value::as_bool).unwrap_or(false);
        if on("authEnabled") {
            Options::Oidc
        } else if on("tokenAuthEnabled") {
            Options::Token
        } else {
            Options::Unknown
        }
    }

    pub fn shows_form(self) -> bool {
        self != Options::Oidc
    }
}

pub struct SignIn {
    pub open: bool,
    pub options: Options,
    pub token: TextEdit,
    pub submitting: bool,
    pub error: Option<String>,
    /// Numbers the questions, so an answer that a newer one has overtaken is dropped.
    asked: u64,
}

impl Default for SignIn {
    fn default() -> SignIn {
        // A secure edit draws bullets, keeps no undo history and wipes what it held when replaced or dropped.
        SignIn {
            open: false,
            options: Options::Loading,
            token: TextEdit::new(String::new()).secure(),
            submitting: false,
            error: None,
            asked: 0,
        }
    }
}

// What is typed must not reach a log through a struct that prints.
impl fmt::Debug for SignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignIn").field("open", &self.open).field("options", &self.options).field("submitting", &self.submitting).finish()
    }
}

impl SignIn {
    pub fn is_loading(&self) -> bool {
        self.options == Options::Loading && !self.submitting
    }

    fn close(&mut self) {
        *self = SignIn { asked: self.asked, ..Default::default() };
    }
}

/// `POST /auth/token-login`: whether the Fleet accepts `token`. Any failure, even no answer, is "Invalid token" in Fleet's page.
pub fn accepts(base: &str, token: &str) -> bool {
    matches!(http::text_with("POST", base, "/auth/token-login", Some(&json!({ "token": token }))), Ok((200..=299, _)))
}

impl State {
    /// The home Fleet refused: show the page and ask what it offers.
    pub fn open_sign_in(&mut self) {
        let Some(link) = self.link.clone() else { return };
        self.sign_in.asked += 1;
        let ask = self.sign_in.asked;
        self.sign_in.open = true;
        self.sign_in.options = Options::Loading;
        self.sign_in.submitting = false;
        link.off_thread(
            |base| http::get(base, "/api/auth/status"),
            move |s, _, answer| {
                if s.sign_in.open && s.sign_in.asked == ask {
                    s.sign_in.options = answer.map_or(Options::Unknown, |status| Options::from_status(&status));
                }
            },
        );
    }

    /// Continue: a blank token is refused without asking; any other is checked by the Fleet.
    pub fn sign_in_submit(&mut self) {
        let Some(link) = self.link.clone() else { return };
        if self.sign_in.submitting || !self.sign_in.options.shows_form() {
            return;
        }
        let Some(token) = auth::Token::new(&self.sign_in.token.text) else {
            self.sign_in.token.replace_text("");
            self.sign_in.error = Some(INVALID.into());
            return;
        };
        // `token.value = trimmedToken`: the field shows what was sent.
        self.sign_in.token.replace_text(token.expose());
        self.sign_in.error = None;
        self.sign_in.submitting = true;
        self.sign_in.asked += 1;
        let ask = self.sign_in.asked;
        link.off_thread(
            move |base| accepts(base, token.expose()).then_some(token),
            move |s, tx, accepted| {
                if !s.sign_in.open || s.sign_in.asked != ask {
                    return;
                }
                s.sign_in.submitting = false;
                match accepted {
                    Some(token) => s.signed_in(token, tx),
                    None => s.sign_in.error = Some(INVALID.into()),
                }
            },
        );
    }

    /// The Fleet took the token: it is the home address's from now on (this run only) and the connection starts again.
    fn signed_in(&mut self, token: auth::Token, _tx: &mut Tx) {
        let Some(old) = self.link.take() else { return };
        auth::set_for(&old.base, Some(token.expose()));
        self.sign_in.close();
        let link = Link::start(old.base.clone(), old.post());
        self.connection = Connection::Connecting;
        self.link = Some(link);
        self.load_pickers();
        self.load_smart_links();
        self.load_list_work();
    }
}
