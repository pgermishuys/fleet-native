//! The machine switch (piece N of docs/parity/17-machines.md): the cases of `use-machine-choice` as the views use it, and
//! `SettingsMachineSwitch.test.ts` under Fleet's names, driven with no window.
use crate::fleet::machine_choice::MachineChoice;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;

type Rt = Runtime<State>;

fn scene(open: bool) -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.show_machine_switch(true, open));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn choice(rt: &Rt) -> &MachineChoice {
    rt.state.machine_switch_scene.as_ref().unwrap()
}

#[test]
fn shows_the_machine_you_re_on_and_pages_ask_it() {
    let mut rt = scene(false);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    let c = choice(&rt);
    assert!(c.enabled(&rt.state.machines));
    assert_eq!(c.key(&rt.state.machines), "m-falcon");
    assert!(c.is_other(&rt.state.machines));
    assert_eq!(c.entry(&rt.state.machines).name, "falcon");
}

#[test]
fn follows_a_pick_until_settings_opens_again_then_starts_from_the_machine_you_re_on() {
    let mut rt = scene(false);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    rt.update(|s, _| s.machine_switch_scene.as_mut().unwrap().pick("home"));
    assert_eq!(choice(&rt).key(&rt.state.machines), "home");
    assert!(!choice(&rt).is_other(&rt.state.machines));
    rt.update(|s, _| s.machine_switch_scene.as_mut().unwrap().reset());
    assert_eq!(choice(&rt).key(&rt.state.machines), "m-falcon");
}

#[test]
fn drops_the_pick_when_its_machine_is_forgotten() {
    let mut rt = scene(false);
    rt.update(|s, _| s.machine_switch_scene.as_mut().unwrap().pick("m-harbor"));
    assert_eq!(choice(&rt).key(&rt.state.machines), "m-harbor");
    rt.update(|s, _| s.machines.list.retain(|m| m.id != "m-harbor"));
    assert_eq!(choice(&rt).key(&rt.state.machines), "home");
}

#[test]
fn each_view_has_its_own_choice() {
    let mut a = MachineChoice::default();
    let b = MachineChoice::default();
    let rt = scene(false);
    a.pick("m-falcon");
    assert_eq!(a.key(&rt.state.machines), "m-falcon");
    assert_eq!(b.key(&rt.state.machines), "home");
}

#[test]
fn is_shown_only_with_another_machine() {
    let mut rt = app();
    rt.update(|s, _| s.show_machine_switch_home_only(true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.rail, Rail::Settings);
    assert!(rt.find("settings-machine-switch").is_none());
    assert!(!choice(&rt).enabled(&rt.state.machines));
    assert_eq!(choice(&rt).key(&rt.state.machines), "home");
    rt.update(|s, _| s.show_machine_switch(true, false));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("settings-machine-switch").is_some());
}

#[test]
fn lists_every_machine_with_its_sessions_and_a_check_on_the_one_shown_picking_one_changes_it() {
    let mut rt = scene(false);
    rt.update(|s, _| s.machines.set_current("m-falcon"));
    click(&mut rt, "settings-machine-switch");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(choice(&rt).open);
    for key in ["home", "m-falcon", "m-harbor", "m-lantern"] {
        assert!(rt.find(&format!("settings-machine-{key}")).is_some(), "no row for {key}");
    }
    let words = |rt: &Rt, key: &str| {
        let all = rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join(" ")
    };
    assert!(words(&rt, "settings-machine-home").contains("2 sessions"), "{}", words(&rt, "settings-machine-home"));
    assert!(words(&rt, "settings-machine-m-falcon").contains("4 sessions"));
    assert!(words(&rt, "settings-machine-m-lantern").contains("0 sessions"));
    assert!(rt.elements().iter().any(|e| e.label.as_deref() == Some("Shown")));
    click(&mut rt, "settings-machine-home");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(choice(&rt).key(&rt.state.machines), "home");
    assert!(!choice(&rt).open, "choosing closes the menu");
}

#[test]
fn says_1_session_for_a_machine_with_one() {
    assert_eq!(crate::view::machine_switch::count_words(1), "1 session");
    assert_eq!(crate::view::machine_switch::count_words(0), "0 sessions");
}
