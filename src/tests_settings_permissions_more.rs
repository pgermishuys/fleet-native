//! Component 16, piece N: Settings → What agents can do, the rows after Run commands. The cases of `PermissionsSection.test.ts`
//! marked "B" in docs/parity/16-settings.md, under their names, against a fake Fleet that answers the preferences and records
//! every `PUT /api/preferences/{key}` and `PUT /api/machines/{id}`.
use crate::Rail;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::machines::AgentWork;
use crate::fleet::settings::permissions_more::Unattended;
use crate::mock::permissions::Case;
use crate::tests::{FRAME, app};
use crate::tests_settings_harnesses::{Fake, Shared};
use crate::tests_settings_permissions::{Rig, rig};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

/// The scene's page on home and three other machines (falcon Always, harbor Never, lantern at Ask), with a Fleet to ask.
fn machines_rig() -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { harnesses: serde_json::json!([]), ..Default::default() }));
    let base = crate::tests_settings_harnesses::serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let mut rt = app();
    rt.set_viewport((1280.0, 900.0), 1.0);
    rt.update(|s, _| {
        s.show_permissions(true, Case::Machines, None);
        s.rail = Rail::Settings;
    });
    rt.frame(FRAME);
    rt.update(move |s, _| {
        s.link = Some(Link::start(
            base,
            Post::new(move |job| {
                let _ = jobs.lock().unwrap().send(job);
            }),
        ));
    });
    let mut r = Rig { rt, queue, fake };
    // A started link reads home's machines at once and the fake has none: let that answer land, then put the scene back.
    r.wait("the link's first read of the machines", |s| s.machines.list.is_empty());
    r.rt.update(|s, _| s.show_permissions(true, Case::Machines, None));
    r.frames();
    r
}

impl Rig {
    fn machine_put(&mut self, id: &str, work: &str) {
        let fake = self.fake.clone();
        let want = (id.to_string(), work.to_string());
        self.wait(&format!("{id} = {work} to reach Fleet"), move |_| fake.lock().unwrap().machine_puts.contains(&want));
    }
    fn order(&self, keys: &[&str]) -> Vec<f32> {
        keys.iter().map(|k| self.info(k).unwrap_or_else(|| panic!("no element keyed {k:?}")).rect[1]).collect()
    }
}

#[test]
fn lists_the_rows_in_order_edits_commands_messages_other_machines_nobody_watching() {
    let r = rig(&[], false);
    let y = r.order(&["reach-row-edits", "reach-row-commands", "reach-row-messages", "reach-row-machines", "reach-row-unattended"]);
    assert!(y.windows(2).all(|w| w[0] < w[1]), "{y:?}");
}

#[test]
fn turns_messages_between_sessions_on_with_the_same_preference_labs_had_for_sessions_started_afterwards() {
    let mut r = rig(&[], false);
    assert!(r.words("reach-row-messages").contains("Applies to sessions started afterwards."));
    assert!(r.on("reach-messages-off") && !r.on("reach-messages-allowed"));
    assert_eq!(r.words("reach-tag-messages"), "default");
    r.click("reach-messages-allowed");
    r.put("SessionMessages", "true");
    assert!(r.on("reach-messages-allowed"));
    assert_eq!(r.words("reach-tag-messages"), "default: Off");
    r.wait("the save to be answered", |s| !s.settings.permissions.messages_saving);
    r.click("reach-messages-off");
    r.put("SessionMessages", "false");
    assert_eq!(r.words("reach-tag-messages"), "default");
}

#[test]
fn says_nowhere_else_that_a_change_waits_for_new_sessions() {
    let r = rig(&[], false);
    let all = r.rt.elements();
    let said = all.iter().filter(|e| e.text.as_deref().is_some_and(|t| t.contains("sessions started afterwards"))).count();
    assert_eq!(said, 1);
}

#[test]
fn points_to_machines_when_there_s_no_other_machine() {
    let mut r = rig(&[], false);
    assert!(r.words("reach-row-machines").contains("No other machines yet."));
    assert!(r.info("reach-add-machines").is_none());
    r.click("reach-add-in-machines");
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Machines);
}

#[test]
fn keeps_unattended_runs_allowed_until_told_otherwise() {
    let mut r = rig(&[], false);
    assert!(r.on("reach-unattended-all"));
    assert_eq!(r.words("reach-tag-unattended"), "default");
    assert!(r.words("reach-row-unattended").contains("Nobody is there to answer, so these runs ask about nothing."));
    r.click("reach-unattended-deny");
    r.put("PermissionUnattended", "deny");
    assert_eq!(r.rt.state.permission_unattended(), Unattended::Refuse);
    assert!(r.words("reach-row-unattended").contains("Anything that would ask is refused, and the agent is told why."));
    assert_eq!(r.words("reach-tag-unattended"), "default: Allow");
    r.click("reach-unattended-same");
    r.put("PermissionUnattended", "same");
    assert!(r.words("reach-row-unattended").contains("A run stops at its first ask and shows Needs you until you answer."));
}

#[test]
fn reads_anything_else_as_allow() {
    let r = rig(&[("PermissionUnattended", "sometimes")], false);
    assert_eq!(r.rt.state.permission_unattended(), Unattended::Allow);
    assert!(r.on("reach-unattended-all"));
}

#[test]
fn lists_machines_set_to_always_or_never_as_exceptions_with_the_level_work_runs_at_there_and_changes_them() {
    let mut r = machines_rig();
    assert!(r.words("reach-machine-m-falcon").contains("runs at Allow edits"));
    assert!(r.words("reach-machine-m-harbor").contains("runs at Ask before changes"));
    assert!(r.info("reach-machine-m-lantern").is_none(), "a machine at Ask is no exception");
    r.click("reach-machine-m-harbor-select");
    assert_eq!(r.words("reach-machine-list-menu"), "AskAlwaysNever");
    r.click("reach-machine-list-opt0");
    r.machine_put("m-harbor", "ask");
    r.wait("harbor back at Ask", |s| s.machines.agent_work.get("m-harbor") == Some(&AgentWork::Ask));
    assert!(r.info("reach-machine-m-harbor").is_none());
    r.click("reach-add-machines");
    assert_eq!(
        r.words("reach-machine-add-list-menu"),
        "harbor \u{b7} Alwaysharbor \u{b7} Neverlantern \u{b7} Alwayslantern \u{b7} Never",
        "each machine at Ask, once as Always and once as Never"
    );
    r.click("reach-machine-add-list-opt3");
    r.machine_put("m-lantern", "never");
    r.wait("lantern at Never", |s| s.machines.agent_work.get("m-lantern") == Some(&AgentWork::Never));
    assert!(r.info("reach-machine-m-lantern").is_some());
}

#[test]
fn a_refused_change_of_a_machine_says_so_under_the_row_and_the_next_try_clears_it() {
    let mut r = machines_rig();
    r.fake.lock().unwrap().refuse.push("m-harbor".into());
    r.click("reach-machine-m-harbor-select");
    r.click("reach-machine-list-opt0");
    r.wait("the refusal", |s| s.permission_machine_error().is_some());
    assert!(r.info("reach-machines-error").is_some());
    assert_eq!(r.rt.state.machines.agent_work.get("m-harbor"), Some(&AgentWork::Never), "nothing changed");
    r.fake.lock().unwrap().refuse.clear();
    r.click("reach-machine-m-harbor-select");
    r.click("reach-machine-list-opt1");
    r.machine_put("m-harbor", "always");
    r.wait("the error gone", |s| s.permission_machine_error().is_none());
}

#[test]
fn another_machine_s_page_says_its_list_is_on_its_own_page() {
    let mut rt = app();
    rt.set_viewport((1280.0, 900.0), 1.0);
    rt.update(|s, _| s.show_permissions(true, Case::Elsewhere, None));
    rt.frame(FRAME);
    rt.frame(FRAME);
    let note = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("reach-machines-live-only")).expect("the note");
    assert_eq!(note.text.as_deref(), Some("Which machines falcon may send work to isn't shown here yet. atlas's list is on its own page."));
    assert!(rt.find("reach-add-machines").is_none());
    assert!(rt.find("reach-machine-m-harbor").is_none());
}
