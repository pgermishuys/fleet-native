//! Component 16, piece C: Settings follows the machine. `SettingsPage.test.ts`'s machine case and `SettingsMachineSwitch`'s
//! placement under Fleet's names, then the chip, the pages that follow the machine and where a page's requests go.
use crate::fleet::link::{Link, Post};
use crate::fleet::settings::machine_scope::{Follows, follows};
use crate::fleet::settings_nav::Page;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;

type Rt = Runtime<State>;

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

/// Settings on `page` with home and three other machines listed, the app on falcon.
fn settings(page: &str, case: Case) -> Rt {
    let mut rt = app();
    rt.set_viewport((1280.0, 900.0), 1.0);
    rt.update(move |s, _| {
        s.show_machines(case, true);
        s.rail = Rail::Sessions;
        s.open_settings(page, None);
    });
    settle(&mut rt);
    rt
}

/// Every word drawn under the element keyed `key`.
fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
}

fn shown(rt: &Rt) -> String {
    rt.state.settings.choice.key(&rt.state.machines).to_string()
}

fn pick(rt: &mut Rt, key: &str) {
    click(rt, "settings-machine-switch");
    settle(rt);
    click(rt, &format!("settings-machine-{key}"));
    settle(rt);
}

#[test]
fn names_the_machine_on_a_page_that_comes_later_and_offers_the_live_machine_s_page() {
    // Fleet's own case: the app is on falcon, so Settings opens on falcon, which General can't ask yet.
    let mut rt = settings("general", Case::Several);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    settle(&mut rt);
    assert_eq!(words(&rt, "settings-machine-later"), "General on falcon comes in a later step.Show atlas's General");
    pick(&mut rt, "m-harbor");
    assert_eq!(words(&rt, "settings-machine-later"), "General on harbor comes in a later step.Show atlas's General");
    assert!(rt.find("general-section").is_none());
    click(&mut rt, "settings-machine-show-live");
    settle(&mut rt);
    assert!(rt.find("settings-machine-later").is_none());
    assert_eq!(shown(&rt), "home");
}

#[test]
fn shows_the_switch_in_the_menu_header_only_with_another_machine_listed() {
    let mut rt = settings("general", Case::Several);
    rt.update(|s, _| s.machines.list.clear());
    settle(&mut rt);
    assert!(rt.find("settings-machine-switch").is_none());
    assert!(rt.find("settings-machine-chip").is_none(), "no chip without a switch");
    let rt = settings("general", Case::Several);
    assert!(rt.find("settings-machine-switch").is_some());
    // Under the "SETTINGS" heading, inside the menu.
    let (_, chip_y) = rt.find("settings-machine-switch").unwrap();
    let (_, first_y) = rt.find("settings-nav-general").unwrap();
    assert!(chip_y < first_y);
}

#[test]
fn opens_on_the_machine_you_re_on_each_time_settings_opens() {
    let mut rt = settings("harnesses", Case::Several);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    pick(&mut rt, "m-harbor");
    assert_eq!(shown(&rt), "m-harbor");
    // Another page of Settings keeps the pick.
    click(&mut rt, "settings-nav-folders");
    settle(&mut rt);
    assert_eq!(shown(&rt), "m-harbor");
    // Leave and come back: the rail's button, an entry point and the palette's command all start again.
    rt.update(|s, _| s.press_rail(Rail::Sessions));
    rt.update(|s, _| s.press_rail(Rail::Settings));
    assert_eq!(shown(&rt), "m-falcon");
    rt.update(|s, _| s.settings.choice.pick("home"));
    rt.update(|s, _| s.rail = Rail::Sessions);
    rt.update(|s, _| s.open_settings("memory", None));
    assert_eq!(shown(&rt), "m-falcon");
    rt.update(|s, _| s.settings.choice.pick("home"));
    rt.update(|s, _| s.open_settings("labs", None));
    assert_eq!(shown(&rt), "home", "opening another page from inside Settings keeps the pick");
}

#[test]
fn says_which_pages_ask_the_machine_which_belong_to_the_app_and_which_come_later() {
    for page in [Page::Harnesses, Page::Folders, Page::Permissions, Page::Skills, Page::Memory, Page::Updates, Page::Credentials] {
        assert_eq!(follows(&page), Follows::Machine, "{page:?}");
    }
    for page in [Page::Appearance, Page::Machines, Page::Plugins, Page::Plugin("github".into())] {
        assert_eq!(follows(&page), Follows::App, "{page:?}");
    }
    for page in [Page::General, Page::Labs] {
        assert_eq!(follows(&page), Follows::Later, "{page:?}");
    }
}

#[test]
fn puts_a_chip_beside_the_title_naming_the_machine_or_this_app() {
    let mut rt = settings("permissions", Case::Several);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    pick(&mut rt, "m-harbor");
    assert_eq!(words(&rt, "settings-machine-chip"), "harbor");
    assert!(rt.find("settings-not-yet").is_some(), "a machine page not built yet still says so, with the chip");
    assert!(rt.find("settings-machine-later").is_none());
    for page in ["appearance", "machines"] {
        rt.update(move |s, _| s.open_settings(page, None));
        settle(&mut rt);
        assert_eq!(words(&rt, "settings-machine-chip"), "This app", "{page}");
        assert!(rt.find("settings-machine-later").is_none(), "{page} is never a later page");
    }
    rt.update(|s, _| s.open_settings("labs", None));
    settle(&mut rt);
    assert_eq!(words(&rt, "settings-machine-chip"), "harbor");
    assert!(rt.find("settings-machine-later").is_some());
}

#[test]
fn a_page_asks_the_machine_shown_not_home() {
    let mut rt = settings("harnesses", Case::Several);
    rt.update(|s, _| {
        s.machines.set_current("m-falcon");
        s.link = Some(Link::start("http://home.example.test:2112".into(), Post::new(|_| ())));
    });
    assert_eq!(rt.state.settings_target().unwrap().base, "http://falcon.example.test:2113", "the machine you're on");
    rt.update(|s, _| s.settings.choice.pick("home"));
    let home = rt.state.settings_target().expect("a link").base.clone();
    assert_eq!(home, "http://home.example.test:2112");
    pick(&mut rt, "m-harbor");
    let target = rt.state.settings_target().expect("a link");
    assert_ne!(target.base, home, "harbor's own address, not home's");
    rt.update(|s, _| s.settings.choice.pick("home"));
    assert_eq!(rt.state.settings_target().unwrap().base, home);
}

#[test]
fn a_pick_of_a_forgotten_machine_falls_back_to_the_one_you_re_on_and_the_page_goes_on() {
    let mut rt = settings("labs", Case::Several);
    pick(&mut rt, "m-harbor");
    assert!(rt.find("settings-machine-later").is_some());
    rt.update(|s, _| s.machines.list.retain(|m| m.id != "m-harbor"));
    settle(&mut rt);
    assert_eq!(shown(&rt), "home", "the machine you're on");
    assert!(rt.find("settings-machine-later").is_none(), "Labs is the page again on the machine you're on");
}

#[test]
fn the_menu_lists_every_machine_and_the_page_header_follows_the_pick() {
    let mut rt = settings("general", Case::Several);
    click(&mut rt, "settings-machine-switch");
    settle(&mut rt);
    for key in ["home", "m-falcon", "m-harbor", "m-lantern"] {
        assert!(rt.find(&format!("settings-machine-{key}")).is_some(), "no row for {key}");
    }
    click(&mut rt, "settings-machine-m-lantern");
    settle(&mut rt);
    assert!(rt.find("settings-machine-m-lantern").is_none(), "choosing closes the menu");
    assert_eq!(words(&rt, "settings-machine-chip"), "lantern");
}
