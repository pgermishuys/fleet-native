//! Scenes of Settings → Updates & about: this Fleet's row in each state it can be in, the app's own row, and the harnesses.
//! Invented versions and numbers throughout; no server is asked.
use crate::State;
use crate::fleet::settings::harness_update::Update as HarnessUpdate;
use crate::fleet::updates::{Status, UpdateStatus};
use crate::update::manifest::Asset;
use crate::update::{Available, Phase};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Nothing waits; the app checked three minutes ago.
    UpToDate,
    /// A Fleet update is out and so is a newer build of the app (the menu shows its dot).
    Available,
    /// Both are coming down.
    Downloading,
    /// This Fleet's update is downloaded; the app is looking.
    Staged,
    /// Both checks failed.
    Failed,
    /// The Fleet is the app's own (no server row) and this build can't update itself.
    Managed,
}

fn status(kind: Status, latest: Option<&str>) -> UpdateStatus {
    UpdateStatus {
        current_version: "0.72.0".into(),
        status: kind,
        latest_version: latest.map(str::to_string),
        checked_at: None,
        error: None,
        download_bytes_received: None,
        download_bytes_total: None,
    }
}

impl State {
    pub fn show_updates(&mut self, dark: bool, case: Case) {
        self.show_harnesses(dark, super::harnesses::Case::Mixed, None);
        let now = crate::mock::rows::NOW;
        // Claude Code has a newer version out, Pi isn't installed: the two things the strip says.
        let mut list = self.feeds.harnesses.entry(crate::fleet::settings::HOME_MACHINE_KEY.to_string()).or_default().harnesses.all.clone();
        if let Some(h) = list.iter_mut().find(|h| h.kind == "claude-code") {
            h.card.update = HarnessUpdate {
                available: true,
                latest_version: Some("2.1.290".into()),
                command: Some("npm install -g @anthropic-ai/claude-code".into()),
                ..HarnessUpdate::default()
            };
        }
        self.feeds.harnesses.entry(crate::fleet::settings::HOME_MACHINE_KEY.to_string()).or_default().harnesses.all = list;
        let offer = || Available {
            build: 214,
            date: "15 November 2026".into(),
            asset: Asset { name: "fleet-native-linux-x64.tar.gz".into(), size: 31_400_000, sha256: "0".repeat(64) },
        };
        self.update.running = Some(210);
        self.update.checked_ms = Some(now - 3 * 60_000);
        let mut server = match case {
            Case::UpToDate => status(Status::UpToDate, None),
            Case::Available => status(Status::Available, Some("0.73.0")),
            Case::Downloading => {
                let mut s = status(Status::Downloading, Some("0.73.0"));
                s.download_bytes_received = Some(5_242_880);
                s.download_bytes_total = Some(20_971_520);
                s
            }
            Case::Staged => status(Status::Staged, Some("0.73.0")),
            Case::Failed => {
                let mut s = status(Status::Error, None);
                s.error = Some("Fleet couldn't reach the release server: the connection was reset.".into());
                s
            }
            Case::Managed => status(Status::Managed, None),
        };
        server.checked_at = None;
        self.updates.status.entry("home".into()).or_default().status = Some(server);
        match case {
            Case::UpToDate => {}
            Case::Available => self.update.available = Some(offer()),
            Case::Downloading => {
                self.update.available = Some(offer());
                self.update.phase = Phase::Downloading(12_400_000, 31_400_000);
            }
            Case::Staged => {
                self.update.checked_ms = None;
                self.update.checking = true;
            }
            Case::Failed => self.update.check_error = Some("The release isn't reachable: the connection was reset.".into()),
            Case::Managed => self.update.running = None,
        }
        self.open_settings("updates", None);
    }
}
