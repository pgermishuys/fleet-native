//! The browser tab's scenes and test fixtures (component 14, piece H): a browser canvas written as the server sends it, with an
//! invented local site. `Fresh` has nothing asked yet, so a fake Fleet is what answers the first question.
use super::pages::Case;
use crate::State;
use crate::fleet::canvases::{read_server_canvas, server_tab_id};
use crate::web::browser::BrowserUi;
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserCase {
    /// A tab with a page of its own, nothing asked of Fleet yet.
    Fresh,
    /// The page is showing, through the preview.
    Shown,
    /// An app's tab with no page yet.
    Waiting,
    /// A tab opened with no address and no app.
    Loading,
    /// Fleet wouldn't show the address.
    Refused,
    /// The session is on another machine that has a web app.
    Elsewhere,
    /// The session is on another machine with no web app.
    ElsewhereNoApp,
    /// Piece I: the app's panel and strip, one per status (and the page shown while it runs).
    AppStarting,
    AppRunning,
    AppExited,
    AppStopped,
    /// The build failed: the output drawer opened by itself.
    AppBuildFailed,
    /// Running, with the output drawer opened by the user and a refused Stop under it.
    AppOutput,
}

impl BrowserCase {
    fn app_status(self) -> Option<&'static str> {
        Some(match self {
            BrowserCase::AppStarting => "starting",
            BrowserCase::AppRunning | BrowserCase::AppOutput => "running",
            BrowserCase::AppExited => "exited",
            BrowserCase::AppStopped => "stopped",
            BrowserCase::AppBuildFailed => "build-failed",
            _ => return None,
        })
    }
}

pub const TAB: &str = "canvas:cv_1";
pub const PAGE: &str = "http://localhost:5173/";
pub const PREVIEW: &str = "http://p1.localhost:41234";

impl State {
    pub fn show_browser_tab(&mut self, case: BrowserCase) {
        self.show_page_pick(Case::Stale);
        let state = match case {
            BrowserCase::Waiting => json!({"url": "", "appId": "app_1"}),
            BrowserCase::Loading => json!({"url": ""}),
            _ => json!({"url": PAGE, "appId": "app_1"}),
        };
        let canvas = json!({"canvasId": "cv_1", "kind": "browser", "title": "Pocket notes", "version": 1, "state": state});
        self.canvases.set_server_canvases("mock.0.0", read_server_canvas(&canvas).into_iter().collect());
        self.canvases.activate("mock.0.0", &server_tab_id("cv_1"));
        self.set_content_open(true);
        let shown = |address: &str| BrowserUi {
            address: rune::TextEdit::new(address),
            proxy_origin: Some(PREVIEW.to_string()),
            target: url::Url::parse(address).ok(),
            frame_src: Some(format!("{PREVIEW}/")),
            wanted: Some(address.to_string()),
            ..Default::default()
        };
        match case {
            // The app cases start from the shown page; `show_app_state` takes the page away where its panel stands in for it.
            BrowserCase::Shown
            | BrowserCase::AppStarting
            | BrowserCase::AppRunning
            | BrowserCase::AppExited
            | BrowserCase::AppStopped
            | BrowserCase::AppBuildFailed
            | BrowserCase::AppOutput => {
                self.web.browsers.insert(TAB.into(), shown(PAGE));
            }
            BrowserCase::Refused => {
                let ui = BrowserUi {
                    error: Some("Fleet couldn't show http://localhost:5173: Nothing is listening on that port.".into()),
                    address: rune::TextEdit::new(PAGE),
                    wanted: Some(PAGE.into()),
                    ..Default::default()
                };
                self.web.browsers.insert(TAB.into(), ui);
            }
            BrowserCase::Waiting | BrowserCase::Loading => {
                self.web.browsers.insert(TAB.into(), BrowserUi { wanted: Some(String::new()), ..Default::default() });
            }
            BrowserCase::Elsewhere | BrowserCase::ElsewhereNoApp => {
                let body = json!({"machines": [{"id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "name": "mini", "baseUrl": "http://10.0.0.7:6262", "token": "tok"}]});
                self.machines.apply_list(crate::fleet::machines::parse_machine_list(200, &body.to_string()));
                self.machines.session_machines.insert("mock.0.0".into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
                if case == BrowserCase::ElsewhereNoApp {
                    let info = crate::fleet::machines::Info::from_json(
                        &json!({"id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "name": "mini", "webApp": false}),
                    );
                    self.machines.identities.extend(info.map(|i| ("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(), i)));
                }
            }
            BrowserCase::Fresh => {}
        }
        if let Some(status) = case.app_status() {
            self.show_app_state(case, status);
        }
    }

    /// An app Fleet runs, in `status`, with the output its command invented.
    fn show_app_state(&mut self, case: BrowserCase, status: &str) {
        let exit = (status == "exited").then_some(1);
        let app = json!({"appId": "app_1", "sessionId": "mock.0.0", "status": status, "command": "npm run dev", "exitCode": exit,
            "url": (status == "running" || status == "build-failed").then_some(PAGE), "ports": [5173, 5174]});
        self.apps.apply_event(&app);
        let lines: Vec<&str> = match status {
            "exited" => vec![
                "old run: listening on 5173",
                crate::fleet::app_runs::RESTART_MARKER,
                "$ npm run dev",
                "> pocket-notes@0.3.1 dev",
                "> vite",
                "Error: Cannot find module './routes'",
                "Require stack:",
                "- src/main.ts",
            ],
            "build-failed" => {
                vec!["$ npm run dev", "> vite", "[vite] Internal server error: Unexpected token (14:3)", "  at src/App.vue:14:3"]
            }
            "starting" => vec![
                "old run: Error: port 5173 is already in use",
                crate::fleet::app_runs::RESTART_MARKER,
                "$ npm run dev",
                "> pocket-notes@0.3.1 dev",
                "> vite",
            ],
            "stopped" => vec!["old run", "Error: crashed", crate::fleet::app_runs::RESTART_MARKER, "$ npm run dev", "> vite"],
            "running" if case == BrowserCase::AppOutput => vec![
                "$ npm run dev",
                "> vite",
                "VITE v5.4.2  ready in 312 ms",
                "Local:   http://localhost:5173/",
                "page reload src/App.vue",
            ],
            _ => Vec::new(),
        };
        self.apps.set_output("app_1", &lines);
        let ui = self.web.browsers.entry(TAB.into()).or_default();
        ui.app_id = Some("app_1".into());
        ui.app.last_status = crate::fleet::app_runs::AppStatus::read(status);
        ui.app.show_logs = matches!(case, BrowserCase::AppBuildFailed | BrowserCase::AppOutput);
        if case == BrowserCase::AppOutput {
            ui.app.action_error = Some(("Fleet couldn't stop the app: it isn't running.".into(), f64::MAX));
        }
        if !matches!(case, BrowserCase::AppRunning | BrowserCase::AppOutput | BrowserCase::AppBuildFailed) {
            ui.target = None;
            ui.frame_src = None;
        }
    }
}
