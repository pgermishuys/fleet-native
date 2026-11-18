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
            BrowserCase::Shown => {
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
    }
}
