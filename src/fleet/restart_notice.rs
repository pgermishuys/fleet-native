//! The "Fleet restarted" notice (`composables/use-restart-notice.ts`): after Fleet restarts with sessions mid-turn, one card
//! says which were cut off, with Continue all and Dismiss all (or Stop all while Fleet is continuing by itself).
//!
//! The card is built by [`build`], a plain function of the cut-off sessions, the other machines' lines and the
//! preference; [`State::keep_restart_notice`] runs it whenever the notices are kept and posts, replaces or removes the
//! card; the buttons tell this module a name and [`State::keep_restart_buttons`] sends the requests off the UI thread.
use super::notices::{Action, ActionTone, IconRef, Item, Link, Notice, Tone};
use super::rows::RowPatch;
use super::settings::switches::RESTART_CONTINUE_AUTO;
use crate::State;
use crate::icons::ROTATE_CW;

/// Every id of this notice starts with this, so one replaces the last (`PREFIX`).
pub const PREFIX: &str = "restart-cut-off:";
/// What the buttons, the link and Esc tell their source, all starting with this.
pub const RUN_PREFIX: &str = "restart:";
const CONTINUE_ALL: &str = "restart:continue-all";
const DISMISS_ALL: &str = "restart:dismiss-all";
const STOP_ALL: &str = "restart:stop-all";
const AUTO_NEXT_TIME: &str = "restart:auto-next-time";
/// How many cut-off sessions the card lists before "+N more" (`LISTED`).
const LISTED: usize = 5;
/// The card stays this long (`holdMs: 30_000`).
const HOLD_SECS: f64 = 30.0;

/// A session a restart cut off, with what the card says of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub id: String,
    pub title: String,
    /// A question was waiting and is lost.
    pub waiting: bool,
    pub at: String,
    /// Fleet is continuing it by itself.
    pub auto: bool,
}

/// "Fleet updated to v0.66.0" after an update, else "Fleet restarted" (`restartTitle`).
pub fn title(updated_to: Option<&str>) -> String {
    match updated_to {
        Some(v) => format!("Fleet updated to v{}", v.strip_prefix('v').unwrap_or(v)),
        None => "Fleet restarted".into(),
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

pub fn cut_off_text(count: usize) -> String {
    format!("{} cut off.", plural(count, "session was", "sessions were"))
}

pub fn continuing_text(count: usize) -> String {
    format!("Continuing {}.", plural(count, "session", "sessions"))
}

/// One line per other machine whose sessions a restart cut off: "build-box restarted: 1 session cut off"
/// (`otherMachineLines`). `counts` is each other machine's name and how many of its sessions are cut off, in order.
pub fn other_machine_lines(counts: &[(String, usize)]) -> Vec<String> {
    counts
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(name, n)| format!("{name} restarted: {} cut off", plural(*n, "session", "sessions")))
        .collect()
}

/// What the notice needs to know, besides the cut-off sessions.
pub struct Inputs<'a> {
    pub cut: &'a [Cut],
    pub lines: &'a [String],
    /// `RestartContinueAuto` is on.
    pub auto_pref: bool,
    pub updated_to: Option<&'a str>,
}

fn plain(text: String) -> Item {
    Item { text, label: None, kind: None }
}

fn action(label: &str, tone: ActionTone, run: &str) -> Action {
    Action { label: label.into(), icon: None, tone, run: run.into() }
}

/// The card for what a restart left, or none when nothing was cut off. `continuing` is the most sessions Fleet was
/// continuing by itself in this restart, so the count holds as the sends go out; it is kept between calls.
pub fn build(inputs: &Inputs, continuing: &mut usize) -> Option<Notice> {
    let (cut, lines) = (inputs.cut, inputs.lines);
    let base = Notice { icon: Some(IconRef(&ROTATE_CW)), tone: Tone::Warn, hold_secs: Some(HOLD_SECS), ..Notice::default() };
    if cut.is_empty() && lines.is_empty() {
        *continuing = 0;
        return None;
    }
    let machine_items = lines.iter().map(|text| plain(text.clone()));
    if cut.is_empty() {
        // Only another machine restarted: that machine's own notice has the buttons.
        let id = format!("{PREFIX}machines:{}", lines.join("|"));
        return Some(merge(base, Notice { items: machine_items.collect(), ..Notice::new(id, "A machine restarted") }));
    }
    let automatic = cut.iter().filter(|c| c.auto).count();
    *continuing = (*continuing).max(automatic);
    let at = cut.iter().map(|c| c.at.as_str()).max().unwrap_or("");
    let listed = cut.iter().take(LISTED).map(|c| {
        let title = c.title.trim();
        let title = if title.is_empty() { "Untitled session" } else { title };
        plain(if c.waiting { format!("{title}: the question was lost. Ask again.") } else { title.to_string() })
    });
    let auto = automatic > 0;
    let notice = Notice {
        body: Some(if auto { continuing_text(*continuing) } else { cut_off_text(cut.len()) }),
        items: listed.chain(machine_items).collect(),
        more: (cut.len() > LISTED).then(|| format!("+{} more", cut.len() - LISTED)),
        link: (!auto && !inputs.auto_pref).then(|| Link {
            label: "Next time, continue these by itself?".into(),
            href: "#".into(),
            run: Some(AUTO_NEXT_TIME.into()),
        }),
        on_escape: auto.then(|| STOP_ALL.to_string()),
        actions: if auto {
            vec![action("Stop all", ActionTone::Danger, STOP_ALL)]
        } else {
            vec![action("Continue all", ActionTone::Primary, CONTINUE_ALL), action("Dismiss all", ActionTone::Quiet, DISMISS_ALL)]
        },
        ..Notice::new(format!("{PREFIX}{at}"), title(inputs.updated_to))
    };
    Some(merge(base, notice))
}

/// `over` on top of what every variant shares.
fn merge(base: Notice, over: Notice) -> Notice {
    Notice { icon: base.icon, tone: base.tone, hold_secs: base.hold_secs, ..over }
}

/// Which sessions the card's buttons act on, kept while it is open.
#[derive(Debug, Default)]
pub struct RestartNotice {
    continuing: usize,
    /// The sessions the card's Continue all and Dismiss all reach.
    ids: Vec<String>,
}

impl RestartNotice {
    /// The most sessions Fleet was continuing by itself in this restart.
    pub fn continuing(&self) -> usize {
        self.continuing
    }
}

impl State {
    /// The home sessions a restart cut off, in list order.
    fn cut_off(&self) -> Vec<Cut> {
        self.projects
            .iter()
            .flat_map(|p| &p.sessions)
            .filter_map(|s| {
                let i = s.row.interrupted.as_ref()?;
                Some(Cut { id: s.id.clone(), title: s.title.clone(), waiting: i.waiting, at: i.at.clone(), auto: i.auto })
            })
            .collect()
    }

    fn other_machine_cut_offs(&self) -> Vec<String> {
        let counts: Vec<(String, usize)> = self
            .machines
            .entries()
            .into_iter()
            .filter(|e| !e.is_home)
            .map(|e| {
                let n = self.machines.others.get(&e.key).map_or(0, |m| m.sessions.iter().filter(|s| s.row.interrupted.is_some()).count());
                (e.name, n)
            })
            .collect();
        other_machine_lines(&counts)
    }

    /// Posts, replaces or removes the card to match what is cut off now (`show`, run on every change in Fleet's watch).
    pub fn keep_restart_notice(&mut self) {
        let cut = self.cut_off();
        let lines = self.other_machine_cut_offs();
        let auto_pref = RESTART_CONTINUE_AUTO.enabled(&self.pickers.prefs);
        let updated_to = self.after_update.updated_to().map(str::to_string);
        let inputs = Inputs { cut: &cut, lines: &lines, auto_pref, updated_to: updated_to.as_deref() };
        let notice = build(&inputs, &mut self.restart.continuing);
        self.restart.ids = cut.into_iter().map(|c| c.id).collect();
        match notice {
            None => self.notices.remove_where(PREFIX, None),
            Some(n) => {
                self.notices.remove_where(PREFIX, Some(&n.id));
                if self.notices.get(&n.id) != Some(&n) {
                    self.notices.post(n, self.clock);
                }
            }
        }
    }

    /// Runs what the card's buttons told: each request on its own thread, all at once, and the card goes when
    /// they're sent (Fleet's `eachSession` then `done`).
    pub fn keep_restart_buttons(&mut self) {
        for run in self.notices.take_heard_with(RUN_PREFIX) {
            match run.as_str() {
                AUTO_NEXT_TIME => self.save_setting(RESTART_CONTINUE_AUTO.key, "true"),
                STOP_ALL => self.stop_automatic(),
                CONTINUE_ALL => self.each_cut_off(true),
                DISMISS_ALL => self.each_cut_off(false),
                _ => {}
            }
        }
    }

    /// Stop all: nothing not yet sent is sent; the cards stay with Continue. Never stops a running turn.
    fn stop_automatic(&mut self) {
        self.restart.continuing = 0;
        if let Some(link) = &self.link {
            link.off_thread(super::http::stop_automatic, |_, _, _| {});
        }
    }

    fn each_cut_off(&mut self, send: bool) {
        let ids = self.restart.ids.clone();
        let Some(link) = self.link.clone() else {
            // No server (a scene or the mock data): the cards are simply taken from the rows.
            for id in ids {
                self.on_row_patch(RowPatch::Interrupted { session: id, interruption: None });
            }
            self.keep_restart_notice();
            return;
        };
        link.off_thread(
            move |base| {
                // All at once, as Fleet's `Promise.allSettled` does: one slow session doesn't hold the rest back.
                std::thread::scope(|scope| {
                    let sent: Vec<_> = ids
                        .iter()
                        .map(|id| {
                            scope.spawn(move || {
                                if send {
                                    super::http::interruption_continue(base, id)
                                } else {
                                    super::http::interruption_dismiss(base, id)
                                }
                            })
                        })
                        .collect();
                    ids.iter()
                        .cloned()
                        .zip(sent)
                        .map(|(id, h)| (id, h.join().unwrap_or_else(|_| Err("the request panicked".into()))))
                        .collect::<Vec<_>>()
                })
            },
            move |s, _, results| {
                // Fleet's event says so too; this covers a socket that's down. A refusal leaves the card on the row.
                for (id, result) in results {
                    if matches!(result, Ok((status, _)) if (200..300).contains(&status) || status == 404) {
                        s.on_row_patch(RowPatch::Interrupted { session: id, interruption: None });
                    }
                }
                s.keep_restart_notice();
            },
        );
    }
}
