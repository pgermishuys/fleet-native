//! The access token of the Fleet running on this same machine, read from the file Fleet keeps it in.
//!
//! A Fleet bound to anything but loopback, or started with `--require-token`, wants a credential on every request,
//! loopback ones too (`Auth/LoopbackAuthPolicy.cs`). Its own machine token is saved next to its database in
//! `<database name>.machine.json` (`MachineIdentityStore.cs`; `fleet.machine.json` for the usual `fleet.db`), as
//! `accessToken`, readable by the current user only. A person running Fleet and this app as the same user on one
//! machine shouldn't have to copy it by hand.
//!
//! Only for a Fleet at a loopback address, only from that file, and the token goes nowhere else: it is handed to
//! `fleet::auth` as home's token, which sends it to home's address alone. It is never printed or logged.
//!
//! Blocking (file reads and HTTP calls): run it on a thread.
use super::discover::{Found, Place};
use super::{auth, http, instance};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The name of the machine file beside the usual database (`fleet.db`).
pub const FILE: &str = "fleet.machine.json";

/// `accessToken` out of a machine file's text; nothing for anything else, or a token shorter than Fleet accepts (16).
pub fn parse_machine_file(json: &str) -> Option<String> {
    let token = serde_json::from_str::<Value>(json).ok()?.get("accessToken")?.as_str()?.trim().to_string();
    (token.len() >= 16).then_some(token)
}

/// Where this Fleet's machine file may be, most likely first: beside the database its instance file names, then
/// the usual file name in every data folder discovery looks in.
pub fn candidates(found: &Found, places: &[Place]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Place::InstanceFile(file) = &found.place
        && let Some(i) = instance::read_instance(file).filter(|i| !i.database_path.is_empty())
    {
        out.push(Path::new(&i.database_path).with_extension("machine.json"));
    }
    for place in places {
        let file = match place {
            Place::InstanceFile(f) => f.parent().map(|d| d.join(FILE)),
            // The desktop app's own settings folder holds no Fleet data.
            Place::SavedPort(_) | Place::Port(_) => None,
        };
        out.extend(file);
    }
    let mut seen = Vec::new();
    out.retain(|p| {
        !seen.contains(p) && {
            seen.push(p.clone());
            true
        }
    });
    out
}

/// What reading the files came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do: a token was given, or the Fleet needs none.
    NotNeeded,
    /// This machine's token was read and Fleet accepted it; `fleet::auth` holds it.
    Adopted,
    /// No file held a token Fleet accepted; the folders looked in, for the message.
    Missing(Vec<PathBuf>),
}

/// Whether the Fleet at `base` turns away a call with no credential (a 401 or 403), by asking for the project list.
fn needs_token(base: &str) -> bool {
    http::get(base, "/api/projects").is_err_and(|why| auth::is_refusal(&why))
}

/// If the Fleet `found` asks for a token and none was given, reads this machine's from Fleet's own file and keeps it
/// as home's, once Fleet has accepted it. `--token` and `FLEET_NATIVE_TOKEN` (already in `fleet::auth`) win.
pub fn adopt(found: &Found, places: &[Place]) -> Outcome {
    if auth::is_set() || !instance::is_loopback_http_url(&found.url) {
        return Outcome::NotNeeded;
    }
    auth::bind_home(&found.url);
    if !needs_token(&found.url) {
        return Outcome::NotNeeded;
    }
    let files = candidates(found, places);
    for file in &files {
        let Some(token) = std::fs::read_to_string(file).ok().and_then(|t| parse_machine_file(&t)) else { continue };
        auth::set(Some(token));
        if !needs_token(&found.url) {
            return Outcome::Adopted;
        }
        auth::set(None);
    }
    let looked: Vec<PathBuf> = files.iter().filter_map(|f| f.parent().map(Path::to_path_buf)).collect();
    auth::note_local_lookup(looked.iter().map(|d| d.display().to_string()).collect());
    Outcome::Missing(looked)
}
