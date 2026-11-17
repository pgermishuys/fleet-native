//! Component 16, piece O: Settings → What agents can do › Browser. Fleet has no test file for `BrowserSection.vue` or
//! `use-agent-browser-settings.ts`, so these are ours (rows 6.38 and 6.40 to 6.53), against a fake Fleet that answers the browser's
//! settings and records every `PUT /api/agent-browser/settings` body.
use crate::fleet::settings::browser::{LOAD_ERROR, Pages, SAVE_ERROR};
use crate::tests_settings_permissions::{Rig, rig, rig_with};
use serde_json::{Value, json};

impl Rig {
    /// The rig with the browser reading as `pages` and `scripts`. They are set before the page opens, so its one read hears them:
    /// a second read in flight could land later and replace a refusal's error or a change.
    fn browser(on: bool, pages: &str, scripts: bool) -> Rig {
        let mut r = rig_with(&[], on, |f| {
            f.browser_pages = pages.to_string();
            f.browser_scripts = scripts;
        });
        r.go_to_browser();
        r.wait("the browser's settings", move |s| {
            let c = s.browser_copy();
            !c.loading
                && c.settings.enabled == on
                && c.settings.scripts == scripts
                && c.settings.pages.value() == if pages.is_empty() { "session" } else { pages }
        });
        r
    }

    /// What a person does: press the Browser chip, and let the page glide to the group.
    fn go_to_browser(&mut self) {
        self.wait("the harnesses", |s| s.permission_harnesses().len() == 3);
        self.click("reach-browser-chip");
        for _ in 0..60 {
            self.rt.frame(crate::tests::FRAME);
        }
    }

    fn browser_puts(&mut self, count: usize) -> Vec<Value> {
        let fake = self.fake.clone();
        self.wait("the change to reach Fleet", move |_| fake.lock().unwrap().browser_puts.len() >= count);
        self.fake.lock().unwrap().browser_puts.clone()
    }

    fn settled(&mut self) {
        self.wait("the save to finish", |s| !s.browser_copy().saving);
    }

    fn opacity(&self, key: &str) -> f32 {
        self.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).opacity
    }
}

#[test]
fn shows_what_the_machine_says_the_browser_is() {
    let r = Rig::browser(true, "machine", true);
    assert_eq!(r.rt.state.browser_copy().settings.pages, Pages::Machine);
    assert!(r.info("browser-section").is_some());
    assert!(r.words("browser-pages-machine").starts_with("Any page on this machine"));
    assert!(r.opacity("browser-enabled-switch") > 0.99 && r.opacity("browser-scripts-switch") > 0.99);
}

#[test]
fn keeps_today_s_defaults_until_the_machine_answers() {
    let s = crate::fleet::settings::browser::Settings::default();
    assert_eq!((s.enabled, s.pages, s.scripts), (true, Pages::Session, false));
}

#[test]
fn sends_only_the_field_that_changed_and_keeps_the_answer() {
    let mut r = Rig::browser(true, "", false);
    r.click("browser-scripts-switch");
    assert_eq!(r.browser_puts(1), vec![json!({"scripts": true})]);
    r.settled();
    assert!(r.rt.state.browser_copy().settings.scripts);
    r.click("browser-pages-any");
    assert_eq!(r.browser_puts(2)[1], json!({"pages": "any"}));
    r.settled();
    assert_eq!(r.rt.state.browser_copy().settings.pages, Pages::Any);
    r.click("browser-enabled-switch");
    assert_eq!(r.browser_puts(3)[2], json!({"enabled": false}));
    r.settled();
    assert!(r.rt.state.browser_off(), "the chip and the harness tabs read the same copy");
    assert_eq!(r.words("reach-browser-chip"), "BrowserOff");
}

#[test]
fn fades_pages_and_scripts_and_lets_neither_change_while_the_browser_is_off() {
    let mut r = Rig::browser(false, "", false);
    assert!(r.opacity("browser-pages-group") < 0.7, "the pages row is at 60%");
    assert!(r.opacity("browser-scripts-switch") < 0.7);
    r.click("browser-pages-any");
    r.click("browser-scripts-switch");
    assert!(r.fake.lock().unwrap().browser_puts.is_empty());
    assert_eq!(r.rt.state.browser_copy().settings.pages, Pages::Session);
}

#[test]
fn never_lets_the_upload_and_trace_rows_change() {
    let mut r = Rig::browser(true, "", false);
    assert!(r.opacity("browser-upload-switch") < 0.7);
    r.click("browser-upload-switch");
    assert!(r.fake.lock().unwrap().browser_puts.is_empty());
    assert!(r.words("browser-settings").contains("Performance traces and memory snapshotsLater"));
}

#[test]
fn puts_the_old_settings_back_and_says_so_when_a_save_is_refused() {
    let mut r = Rig::browser(true, "", false);
    r.fake.lock().unwrap().browser_refuse = true;
    r.click("browser-scripts-switch");
    r.browser_puts(1);
    r.wait("the refusal", |s| s.browser_copy().error.is_some());
    assert!(!r.rt.state.browser_copy().settings.scripts);
    assert_eq!(r.words("browser-error"), SAVE_ERROR);
    // The next try clears it.
    r.fake.lock().unwrap().browser_refuse = false;
    r.click("browser-scripts-switch");
    r.wait("the answer", |s| s.browser_copy().settings.scripts);
    assert!(r.info("browser-error").is_none());
}

#[test]
fn says_so_when_the_settings_can_t_be_read_and_keeps_the_defaults() {
    let mut r = rig_with(&[], true, |f| f.browser_garbage = true);
    r.go_to_browser();
    r.wait("the unreadable answer", |s| s.browser_copy().error.is_some());
    assert_eq!(r.words("browser-error"), LOAD_ERROR);
    let c = r.rt.state.browser_copy();
    assert!((c.settings.enabled, c.settings.pages, c.settings.scripts) == (true, Pages::Session, false));
}

#[test]
fn waits_for_the_machine_before_the_controls_work() {
    let mut r = rig(&[], true);
    r.go_to_browser();
    // The page's own read must be answered first, or its reply would clear the loading state set below.
    r.wait("the page's read", |s| !s.browser_copy().loading);
    r.rt.update(|s, _| s.settings.browser.entry("home".into()).or_default().loading = true);
    r.frames();
    r.click("browser-scripts-switch");
    assert!(r.fake.lock().unwrap().browser_puts.is_empty());
    assert!(r.opacity("browser-scripts-switch") < 0.7);
}

#[test]
fn keeps_one_copy_per_machine() {
    let mut r = Rig::browser(true, "", false);
    r.rt.update(|s, _| {
        let mut other = crate::fleet::settings::browser::Copy::default();
        other.settings.enabled = false;
        s.settings.browser.insert("elsewhere".into(), other);
    });
    assert!(!r.rt.state.browser_off(), "home's copy is untouched");
}

#[test]
fn opens_the_browser_group_from_the_chip_and_scrolls_to_it_once() {
    let mut r = rig(&[], true);
    r.go_to_browser();
    assert_eq!(r.rt.state.settings.anchor, None, "the anchor is spent once it has scrolled");
    let (group, view) = (r.info("settings-anchor-browser").expect("the group"), r.info("permissions-section").expect("the scroll"));
    // The page is only a little taller than the window, so it can't put the group's top at the top: it scrolls as far as it goes,
    // and the whole group shows.
    let (top, bottom) = (group.rect[1], group.rect[1] + group.rect[3]);
    assert!(top >= view.rect[1] && bottom <= view.rect[1] + view.rect[3] + 1.0, "the group {:?} in {:?}", group.rect, view.rect);
}
