//! Export CSV (`AnalyticsNavPanel.vue`, `use-analytics-menu.ts` `exportHref` and `exportCsv`). Fleet's own machine is a plain
//! download link and another machine's is fetched with its key and then saved; either way the file is `analytics.csv`.
//! Here both are fetched on a thread first, so a refusal does nothing (`if (!response.ok) return`), then the system's save
//! dialog is offered the name, and the bytes the server sent are written on a thread. The CSV is the server's: nothing here
//! reads or rewrites it.
use super::export_path;
use crate::State;
use crate::fleet::summary::Target;
use rune::{FileDialogOptions, Tx};

/// The name the server's own download gives it.
pub const FILE_NAME: &str = "analytics.csv";

/// The body of `path` on `target` as text; a status outside 2xx is an error.
fn fetch(target: &Target, path: &str) -> Result<String, String> {
    let (status, body) = match target {
        Target::Here(base) => crate::fleet::http::get_page(base, path)?,
        Target::There(machine) => crate::fleet::machine_routes::get_on(machine, path).map_err(|e| e.to_string())?,
    };
    if (200..300).contains(&status) { Ok(body) } else { Err(format!("HTTP {status}")) }
}

impl State {
    /// Export CSV pressed: reads the CSV for the filters as they are now. One export at a time.
    pub fn analytics_export(&mut self) {
        if self.analytics.exporting {
            return;
        }
        let key = self.analytics.machine().map(str::to_string).unwrap_or_else(|| self.analytics_machine());
        let Some(target) = self.analytics_target(&key) else { return };
        let Some(post) = self.link.as_ref().map(|l| l.post()) else { return };
        let path = export_path(&self.analytics.filters);
        self.analytics.exporting = true;
        self.analytics.exports_asked += 1;
        std::thread::spawn(move || {
            let body = fetch(&target, &path);
            post.update(move |s, tx| s.analytics_exported(body, tx));
        });
    }

    /// The CSV came back (or didn't): a refusal ends here, nothing shown; else the save dialog is offered the name.
    fn analytics_exported(&mut self, body: Result<String, String>, tx: &mut Tx) {
        let Ok(csv) = body else {
            self.analytics.exporting = false;
            return;
        };
        let options = FileDialogOptions { title: "Export CSV".into(), file_name: Some(FILE_NAME.into()), ..Default::default() }
            .filter("CSV", &["csv"]);
        tx.save_file(options, move |s: &mut State, answer, tx| {
            s.analytics.exporting = false;
            if let Ok(Some(path)) = answer {
                s.save_text(FILE_NAME.into(), path, csv, tx);
            }
        });
    }
}
