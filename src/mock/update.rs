//! The updater's scenes: a newer build is known (with invented numbers), and the dialog in each of its states.
use crate::State;
use crate::update::manifest::Asset;
use crate::update::{Available, Phase};

#[derive(Clone, Copy)]
pub enum Case {
    /// Only the status bar's item.
    Item,
    Dialog,
    Downloading,
    Failed,
    Manual,
    /// Installed from a `.deb`: the dialog only links.
    Deb,
}

pub fn show(s: &mut State, case: Case) {
    s.update.available = Some(Available {
        build: 214,
        date: "15 November 2026".into(),
        asset: Asset { name: "fleet-native-linux-x64.tar.gz".into(), size: 131_400_000, sha256: "0".repeat(64) },
        link: matches!(case, Case::Deb).then(|| "https://example.invalid/latest/fleet-native-linux-x64.deb".to_string()),
    });
    s.update.dialog = !matches!(case, Case::Item);
    s.update.phase = match case {
        Case::Item | Case::Dialog | Case::Deb => Phase::Idle,
        Case::Downloading => Phase::Downloading(12_400_000, 31_400_000),
        Case::Failed => Phase::Failed(
            "The download doesn't match its checksum, so it was thrown away (expected 3f9a1c7e02bd, got 91d4e8a60c35).".into(),
        ),
        Case::Manual => Phase::Manual {
            why:
                "/Applications can't be written to by this app. The disk image is open: drag Fleet Native over the old one in Applications."
                    .into(),
        },
    };
}
