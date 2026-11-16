//! The line under the conversation while a turn runs. Ports `components/session/WorkingIndicator.vue` and the props
//! `ActivityStream.vue` gives it. `conv::working` says what it reads; this draws it.
//! Fleet sweeps a highlight through the word "Working"; Rune can't clip a gradient to glyphs, so the word stays plain
//! muted, as Fleet draws it under reduced motion (checklist, row 1.78).
use super::message::now_ms;
use super::parts::{diamond, quad};
use crate::State;
use crate::composer::status::SteerDelivery;
use crate::conv::working::{Inputs, Working, WorkingOn, turn_started_at, working_line, working_on};
use crate::mock::Status;
use crate::theme::{Theme, sans};
use rune::{El, el, row, text};

/// The row's box (a virtual row takes the key of its own, so the line's key is on the box inside): `display:flex; align-items:center; gap:10; height:24; 13px muted`, inside
/// `.streaming-indicator` (`margin 0 auto 12; padding 4px 0`).
fn frame() -> El {
    // Fleet leaves 12 + 12 below the line (its margin and the scroll box's padding); the conversation's own bottom padding
    // is 30, made for a message, so the line takes 6 back.
    row().w_full().py(4.0).mb(-6.0)
}

/// The idle glyph, a hollow ring (`StatusGlyph` `idle`): 8px, stroke 1.5, radius 3.
fn ring(t: &Theme) -> El {
    el().size(8.0, 8.0).center().child(el().size(6.0, 6.0).radius(3.0).border(1.5, t.muted).border_layout())
}

/// What the open session's line is told, as `ActivityStream.vue` computes it.
pub struct Facts {
    pub since: Option<i64>,
    pub waiting: bool,
    pub work: Option<WorkingOn>,
}

impl State {
    /// The turn's start, whether a question holds it, and the call it is running.
    pub fn working_facts(&self) -> Facts {
        let session = self.session();
        let waiting = session.status == Status::NeedsInput || session.row.activity == "waiting_input";
        let next_step = self.caps().steer_delivery == Some(SteerDelivery::NextStep);
        match &self.stream {
            Some(stream) => Facts {
                since: turn_started_at(&stream.messages).or_else(|| self.demo_turn_start()),
                waiting,
                work: working_on(&stream.messages, next_step),
            },
            None => Facts { since: self.demo_turn_start(), waiting, work: None },
        }
    }

    /// The invented sessions have no prompt times: their turn began `working_since` on the app's own clock.
    fn demo_turn_start(&self) -> Option<i64> {
        if self.link.is_some() {
            return None;
        }
        Some(now_ms(self) - ((self.clock - self.working_since).max(0.0) * 1000.0) as i64)
    }

    /// The name of the open session's machine while it isn't answering: the turn it was running can't be known to run on
    /// (`quietMachine` in `ActivityStream.vue`). The scenes set `quiet_machine` themselves.
    pub fn not_answering_machine(&self) -> Option<String> {
        if self.quiet_machine.is_some() {
            return self.quiet_machine.clone();
        }
        let key = self.machine_key_of(&self.open_id()?);
        if self.machines.is_answering(key) {
            return None;
        }
        Some(self.machines.machine(key).map_or_else(|| "Its machine".to_string(), |m| m.name.clone()))
    }

    /// What the line says right now, and the second it changes (for `memo`-less redraws the poll already does each
    /// second; this is for the tests and the scenes).
    pub fn working_text(&self) -> Working {
        let facts = self.working_facts();
        let retrying = self.session().row.activity == "retry";
        let quiet = self.not_answering_machine();
        let inputs = Inputs {
            since: facts.since,
            waiting: facts.waiting,
            retry: retrying.then_some(&self.session().row.retrying),
            not_answering: quiet.as_deref(),
            work: facts.work.as_ref(),
        };
        working_line(&inputs, now_ms(self))
    }
}

/// `WorkingIndicator` (rows 1.77-1.82).
pub fn line(t: &Theme, s: &State) -> El {
    let (key, glyph, words) = match s.working_text() {
        Working::Waiting => (
            "working-indicator",
            diamond(t.idle),
            text("Needs input").key("working-waiting").font(sans(13.0, 500.0, 1.3)).color(t.idle_text),
        ),
        Working::NotAnswering(name) => (
            "working-not-answering",
            ring(t),
            text(format!("{name} isn't answering. It was working when last heard.")).font(sans(13.0, 400.0, 1.3)).color(t.muted).wrap(),
        ),
        Working::Retry(said) => (
            "working-indicator",
            quad(t.idle),
            // Why and when can be long: it wraps rather than cutting off the reason.
            text(said).key("working-retry").font(sans(13.0, 400.0, 1.45)).color(t.idle_text).wrap().shrink().min_w(0.0),
        ),
        Working::Busy { elapsed, tool } => {
            let font = sans(13.0, 400.0, 1.3).features("'tnum' 1");
            let mut words = row().gap(10.0).center_y().child(text("Working").key("working-word").font(font).color(t.muted));
            // `.working__elapsed` pulls itself 5px toward the word, so its 10px gap reads as 5.
            for (key, said) in [("working-elapsed", elapsed), ("working-tool", tool)] {
                if let Some(said) = said {
                    words = words.child(text(format!("\u{b7} {said}")).key(key).font(font).color(t.muted).ml(-5.0));
                }
            }
            ("working-indicator", quad(t.text), words)
        }
    };
    let tall = key == "working-indicator" && s.session().row.activity == "retry";
    let inner = row().gap(10.0).center_y().shrink().min_w(0.0);
    let inner = if tall { inner.min_h(24.0) } else { inner.h(24.0) };
    frame().child(inner.key(key).child(glyph).child(words))
}
