//! Component 20, piece E: Export CSV and the date fields' calendar (`AnalyticsNavPanel.vue`, `use-analytics-menu.ts`,
//! `AnalyticsFilters.vue`). Fleet's `AnalyticsNavPanel.test.ts` fourth test under its name; the rest are the native rules of
//! rows 3.5, 3.6 and 4.1 to 4.3. Export writes only into a scratch folder, through the save dialog's answer.
use crate::fleet::analytics::filters::Filters;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app, click, press_key};
use crate::{Rail, State, test_wait};
use rune::{FileDialogKind, Key, Mods, Runtime};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// 20 November 2026, noon UTC.
const NOW_MS: i64 = 20_777 * 86_400_000 + 43_200_000;
const CSV: &str = "id,event_id,session_id,project_id,project_name,workspace_directory,model_id,provider_id,tokens_input,tokens_output,tokens_reasoning,tokens_cache_read,tokens_cache_write,tokens_total,cost,estimated_cost,created_at\n1,e1,s1,p1,harbor-api,/work/harbor,model-a,prov,10,20,0,0,0,30,0.5,0.5,2026-11-19T10:00:00Z\n";

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics(true, scene));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn filters(rt: &Runtime<State>) -> Filters {
    rt.state.analytics.filters.clone()
}

#[test]
fn has_the_from_to_project_filters_and_export_below_the_sections_and_no_by_harness() {
    let rt = page(Scene::Filters);
    for key in ["analytics-filter-from", "analytics-filter-to", "analytics-project", "analytics-export"] {
        assert!(shown(&rt, key), "{key} is drawn");
    }
    assert!(!shown(&rt, "analytics-section-harness") && !shown(&rt, "analytics-filter-harness"), "no By harness");
    let below = |key: &str| rt.find(key).unwrap().1;
    let last_section = below("analytics-section-sessions");
    for key in ["analytics-filter-from", "analytics-project", "analytics-export"] {
        assert!(below(key) > last_section, "{key} is under the sections");
    }
    assert!(below("analytics-export") > below("analytics-project"), "Export is the last of the block");
}

// ─── the calendar ─────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_glyph_opens_the_calendar_on_the_fields_month_and_pressing_it_again_closes_it() {
    let mut rt = page(Scene::Filters);
    assert!(!shown(&rt, "analytics-calendar-menu"));
    click(&mut rt, "analytics-filter-from-calendar");
    assert!(shown(&rt, "analytics-calendar-menu"));
    assert!(shown(&rt, "analytics-calendar-day-2026-10-21"), "From holds 21 October, so October is shown");
    click(&mut rt, "analytics-calendar-next");
    assert!(shown(&rt, "analytics-calendar-day-2026-11-20"), "next month");
    click(&mut rt, "analytics-calendar-prev");
    click(&mut rt, "analytics-calendar-prev");
    assert!(shown(&rt, "analytics-calendar-day-2026-09-30"), "back a month: September's days");
}

#[test]
fn escape_and_a_press_outside_close_the_calendar_and_change_nothing() {
    let mut rt = page(Scene::Filters);
    let before = filters(&rt);
    click(&mut rt, "analytics-filter-to-calendar");
    assert!(shown(&rt, "analytics-calendar-menu"));
    press_key(&mut rt, Key::Escape, Mods::default());
    assert!(!shown(&rt, "analytics-calendar-menu"), "Escape closes it");
    click(&mut rt, "analytics-filter-to-calendar");
    click(&mut rt, "analytics-calendar-scrim");
    assert!(!shown(&rt, "analytics-calendar-menu"), "a press outside closes it");
    assert_eq!(filters(&rt), before);
}

#[test]
fn a_day_picked_sets_the_field_and_closes_the_calendar() {
    let mut rt = page(Scene::Filters);
    click(&mut rt, "analytics-filter-from-calendar");
    click(&mut rt, "analytics-calendar-day-2026-11-05");
    assert!(!shown(&rt, "analytics-calendar-menu"));
    assert_eq!(filters(&rt), Filters { from: "2026-11-05".into(), to: "2026-11-20".into(), project_id: String::new() });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.fields.from.text, "11/05/2026", "the field shows it");
    click(&mut rt, "analytics-filter-to-calendar");
    click(&mut rt, "analytics-calendar-day-2026-11-12");
    assert_eq!(filters(&rt).to, "2026-11-12");
}

#[test]
fn the_pickers_limits_are_from_up_to_to_and_to_down_to_from() {
    let mut rt = page(Scene::Filters);
    // From holds 21 October and To 20 November.
    click(&mut rt, "analytics-filter-from-calendar");
    click(&mut rt, "analytics-calendar-next");
    let before = filters(&rt);
    click(&mut rt, "analytics-calendar-day-2026-11-25");
    assert_eq!(filters(&rt), before, "From can't be picked past To");
    assert!(shown(&rt, "analytics-calendar-menu"), "and the calendar stays open");
    click(&mut rt, "analytics-calendar-day-2026-11-20");
    assert_eq!(filters(&rt).from, "2026-11-20", "To's own day is allowed");
    click(&mut rt, "analytics-filter-to-calendar");
    click(&mut rt, "analytics-calendar-day-2026-11-19");
    assert_eq!(filters(&rt).to, "2026-11-20", "To can't be picked before From");
}

// ─── Export CSV ──────────────────────────────────────────────────────────────────────────────────────────────────────────

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    heard: Arc<Mutex<Vec<String>>>,
    folder: PathBuf,
}

/// The app connected to a fake Fleet that answers the export with `export` (a status and a body) and the reads with nothing.
fn rig(name: &str, export: (u16, &'static str)) -> Rig {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.route() == "/api/analytics/export" {
                    log.lock().unwrap().push(req.path.clone());
                    return crate::test_http::respond(&mut stream, export.0, "text/csv", export.1);
                }
                let body = if req.route() == "/api/analytics/summary" { "{}" } else { "[]" };
                crate::test_http::respond_json(&mut stream, "200 X", body);
            });
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start_machine(HOME_MACHINE_KEY.to_string(), base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.now_ms = Some(NOW_MS);
        s.rail = Rail::Analytics;
    });
    rt.frame(FRAME);
    let folder = std::env::temp_dir().join(format!("fleet-native-20e-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    Rig { rt, queue, heard, folder }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!("exporting {}, asked {}", s.analytics.exporting, s.analytics.exports_asked)
        });
    }

    fn exports(&self) -> Vec<String> {
        self.heard.lock().unwrap().clone()
    }

    /// Waits for the save dialog the export's answer opens; its kind and the name it offers.
    fn dialog(&mut self) -> (FileDialogKind, Option<String>) {
        let queue = &self.queue;
        let mut seen = None;
        test_wait::until(
            &mut self.rt,
            "the save dialog",
            |rt| {
                while let Ok(job) = queue.try_recv() {
                    rt.update(job);
                }
                rt.frame(FRAME);
                seen = rt.file_dialog().map(|(kind, options)| (kind, options.file_name.clone()));
                seen.is_some()
            },
            |_| "no dialog".into(),
        );
        seen.unwrap()
    }

    /// The reads of the first look have all answered, so only the export is left to count.
    fn settled(&mut self) {
        self.wait("the first reads", |s| {
            let a = &s.analytics;
            !(a.summary.is_loading || a.range.is_loading || a.daily.is_loading || a.models.is_loading || a.sessions.is_loading)
        });
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

#[test]
fn export_asks_the_export_path_with_the_current_filters_offers_analytics_csv_and_writes_the_servers_bytes() {
    let mut r = rig("ok", (200, CSV));
    r.settled();
    r.rt.update(|s, _| s.analytics_pick_project("harbor-api"));
    click(&mut r.rt, "analytics-export");
    // The save dialog is the answer to the fetch: wait for it before asserting anything it holds.
    let asked = r.heard.clone();
    r.wait("the export request", move |_| !asked.lock().unwrap().is_empty());
    let (kind, name) = r.dialog();
    assert_eq!(r.exports(), ["/api/analytics/export?format=csv&from=2026-10-21&to=2026-11-20&projectId=harbor-api"]);
    assert_eq!(kind, FileDialogKind::Save);
    assert_eq!(name.as_deref(), Some("analytics.csv"));
    let path = r.folder.join("analytics.csv");
    r.rt.resolve_file_dialog(Some(path.clone())).unwrap();
    let wrote = path.clone();
    r.wait("the file and its toast", move |s| wrote.exists() && s.ui.toast.is_some() && !s.analytics.exporting);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CSV, "the server's bytes, as they came");
}

#[test]
fn a_cancelled_save_dialog_writes_nothing_and_lets_the_next_export_run() {
    let mut r = rig("cancel", (200, CSV));
    r.settled();
    click(&mut r.rt, "analytics-export");
    r.dialog();
    r.rt.resolve_file_dialog(None).unwrap();
    r.wait("the export to end", |s| !s.analytics.exporting);
    assert_eq!(r.folder.read_dir().unwrap().count(), 0, "nothing was written");
}

#[test]
fn a_refused_export_writes_nothing_and_shows_nothing() {
    let mut r = rig("refused", (503, "Analytics is disabled."));
    r.settled();
    click(&mut r.rt, "analytics-export");
    r.wait("the export to be asked and refused", |s| s.analytics.exports_asked == 1 && !s.analytics.exporting);
    assert_eq!(r.exports().len(), 1, "it asked once");
    assert!(r.rt.file_dialog().is_none(), "no save dialog is offered");
    assert!(r.rt.state.ui.toast.is_none(), "nothing is said");
    assert_eq!(r.folder.read_dir().unwrap().count(), 0);
}

#[test]
fn a_second_press_while_the_first_export_is_open_asks_nothing_more() {
    let mut r = rig("twice", (200, CSV));
    r.settled();
    r.rt.update(|s, _| {
        s.analytics_export();
        s.analytics_export();
    });
    assert_eq!(r.rt.state.analytics.exports_asked, 1);
}
