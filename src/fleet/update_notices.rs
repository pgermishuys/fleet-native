//! Machine update notices (`composables/use-fleet-update-notices.ts`): one card when a machine has updated itself ("falcon
//! updated to 0.59.1"), and one chip in the status bar that counts the machines that can update, or says where a rollout is,
//! and opens Settings → Updates & about › Machines.
//!
//! The words come from `fleet::updates::rows` (`describe_updated_notice`, `describe_update_chip`); this file decides what is
//! posted. [`card`] and [`chip`] are plain functions; [`State::keep_update_notices`] runs them whenever the notices are kept,
//! and the buttons tell this module a name that [`State::keep_update_buttons`] acts on.
use super::notices::{Action, ActionTone, Notice};
use super::updates::after_update;
use super::updates::rows::{UpdatedNotice, describe_update_chip, describe_updated_notice, has_update_waiting};
use super::updates::{
    Rollout, Trigger, UPDATE_RULE_DEFAULT_KEY, UPDATE_RULE_NOTICE_KEY, UPDATE_RULE_OVERRIDES_KEY, UpdateRule, parse_chosen_rule,
    parse_rule_overrides,
};
use crate::State;
use crate::conv::time::local_offset_secs;

/// The one chip's id (`CHIP_ID`).
pub const CHIP_ID: &str = "fleet-updates-waiting";
/// Every id of the updated card starts with this.
pub const UPDATED_PREFIX: &str = "fleet-updated:";
/// What the buttons and the chip tell their source, all starting with this.
pub const RUN_PREFIX: &str = "fleet-update:";
const OPEN_MACHINES: &str = "fleet-update:open-machines";
const WHATS_NEW: &str = "fleet-update:whats-new:";
const CHANGE_RULE: &str = "fleet-update:change-rule:";
const DISMISS: &str = "fleet-update:dismiss:";
/// Said once, after the sentence about when, to a user who never chose a rule.
const UNPICKED_RULE: &str = "Nobody chose an update rule, so it updates itself when nothing is in use. You can change that.";

/// What the notices need to know of the preferences.
pub struct Rules<'a> {
    /// The first answer from the server has come (`hasFetched`).
    pub fetched: bool,
    /// `FleetUpdateRuleNotice` is "true": the user was told already.
    pub told: bool,
    pub chosen: Option<UpdateRule>,
    pub overrides: &'a std::collections::BTreeMap<String, UpdateRule>,
}

/// A machine that restarted itself under a rule nobody picked: the user is told so once, ever (`unpickedRuleNews`).
pub fn unpicked_rule_news(key: &str, trigger: Trigger, rules: &Rules) -> bool {
    if !matches!(trigger, Trigger::Idle | Trigger::Overnight) || !rules.fetched || rules.told {
        return false;
    }
    rules.chosen.is_none() && !rules.overrides.contains_key(key)
}

/// The card for a machine that updated: What's new leads, "Change the rule" only when the rule sentence is said.
pub fn card(news: &UpdatedNotice, told_rule: bool) -> Notice {
    let action = |label: &str, tone: ActionTone, run: String| Action { label: label.into(), icon: None, tone, run };
    let mut actions = vec![action("What's new", ActionTone::Primary, format!("{WHATS_NEW}{}:{}", news.id, news.version))];
    if told_rule {
        actions.push(action("Change the rule", ActionTone::Quiet, format!("{CHANGE_RULE}{}", news.id)));
    }
    actions.push(action("Dismiss", ActionTone::Quiet, format!("{DISMISS}{}", news.id)));
    Notice {
        body: Some(if told_rule { format!("{} {UNPICKED_RULE}", news.body) } else { news.body.clone() }),
        actions,
        ..Notice::new(news.id.clone(), news.title.clone())
    }
}

/// The one chip, or none (`watch([waiting, rollout])`): only ever a chip, and a click goes to the Machines group.
pub fn chip(waiting: usize, rollout: Option<&Rollout>) -> Option<Notice> {
    let text = describe_update_chip(waiting, rollout)?;
    Some(Notice { on_chip_click: Some(OPEN_MACHINES.into()), ..Notice::new(CHIP_ID, text.clone()).chip(text).quiet() })
}

/// What the notices have asked of the server so far.
#[derive(Debug, Default)]
pub struct UpdateNotices {
    /// The rollout has been read once, so the chip knows of one that was under way before the app started.
    rollout_read: bool,
}

impl State {
    /// Posts the updated card of each machine that has news, and keeps the chip to match what can update now. Run on every
    /// change in the notices; a card whose id is in the store already posts nothing (`notices.has`), and one that was shown
    /// before comes back as no card (the store's own rule).
    pub fn keep_update_notices(&mut self) {
        self.keep_after_update();
        if !self.update_notices.rollout_read && self.link.is_some() {
            self.update_notices.rollout_read = true;
            self.load_rollout();
        }
        let waiting = self.update_rows().iter().filter(|r| has_update_waiting(&r.state)).count();
        match chip(waiting, self.updates.rollout.as_ref()) {
            None => self.notices.remove(CHIP_ID),
            Some(n) if self.notices.get(CHIP_ID) != Some(&n) => self.notices.post(n, self.clock),
            Some(_) => {}
        }
        let prefs = &self.pickers.prefs;
        let overrides = parse_rule_overrides(prefs.get(UPDATE_RULE_OVERRIDES_KEY));
        let chosen = parse_chosen_rule(prefs.get(UPDATE_RULE_DEFAULT_KEY));
        let (fetched, mut told) = (prefs.fetched, prefs.get(UPDATE_RULE_NOTICE_KEY) == Some("true"));
        let now = crate::view::message::now_ms(self);
        let mut cards = Vec::new();
        for entry in self.machines.entries() {
            let info = entry.info.as_ref();
            let last = info.and_then(|i| i.update.as_ref()).and_then(|u| u.last_update.as_ref());
            let running = info.map(|i| i.version.as_str()).filter(|v| !v.is_empty());
            let Some(news) = describe_updated_notice(&entry.key, &entry.name, running, last, now, local_offset_secs()) else { continue };
            if self.notices.has(&news.id) {
                continue;
            }
            // The rule sentence is said to the first card only: once it is, `told` holds for the rest.
            let rules = Rules { fetched, told, chosen, overrides: &overrides };
            let told_rule = unpicked_rule_news(&entry.key, news.trigger, &rules);
            told |= told_rule;
            cards.push((card(&news, told_rule), told_rule));
        }
        for (notice, told_rule) in cards {
            if told_rule {
                self.save_setting(UPDATE_RULE_NOTICE_KEY, "true");
            }
            self.notices.post(notice, self.clock);
        }
    }

    /// Fleet's `settle(id)`: the card closes into its chip, or goes when it has none; one that isn't open just goes.
    fn settle_update_card(&mut self, id: &str) {
        if self.notices.open().is_some_and(|n| n.id == id) {
            self.notices.settle();
        } else {
            self.notices.remove(id);
        }
    }

    /// Acts on what the card's buttons and the chip told.
    pub fn keep_update_buttons(&mut self) {
        for run in self.notices.take_heard_with(RUN_PREFIX) {
            if run == OPEN_MACHINES {
                self.open_settings("updates", Some("machines"));
            } else if let Some(rest) = run.strip_prefix(WHATS_NEW) {
                // What's new opens at that version (`openWhatsNew(version)`), and the card closes into its chip.
                let (id, version) = rest.rsplit_once(':').unwrap_or((rest, ""));
                self.open_whats_new((!version.is_empty()).then_some(version), None);
                self.settle_update_card(id);
            } else if let Some(rest) = run.strip_prefix(after_update::OPEN_CHIP) {
                let (version, since) = after_update::parse_open(rest);
                self.open_whats_new(Some(version), since);
            } else if let Some(rest) = run.strip_prefix(after_update::OPEN_CARD) {
                let (version, since) = after_update::parse_open(rest);
                self.open_whats_new(Some(version), since);
                self.settle_update_card(&after_update::card_id(version));
            } else if let Some(id) = run.strip_prefix(CHANGE_RULE) {
                self.open_settings("updates", Some("machines"));
                self.settle_update_card(id);
            } else if let Some(id) = run.strip_prefix(DISMISS) {
                self.settle_update_card(id);
            }
        }
    }
}
