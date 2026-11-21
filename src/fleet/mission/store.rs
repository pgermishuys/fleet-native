//! Mission control kept live (component 23, piece C; `composables/use-mission-control.ts`): a feed for each machine the
//! app knows, opened while the Fleet view is shown and closed the moment it isn't. Each feed is `feed::Feed` (when to read)
//! and `read` (what a read brings); this is what opens them, syncs them with the machines list, and applies their answers.
//!
//! Nothing here is the owner's undecided "Keep every machine live" (17.25, 17.26): that would keep the feeds open for the
//! session list too. These open for Fleet's own view and go with it.
//!
//! Every call runs on a thread and is handed back with `Post::update`; the hub's news (a machine's hub up or down, a
//! `sessions` event) is noted by the link's thread and applied at the next poll, with the app's clock.
use super::feed::Feed;
use super::read::{Inbox, Reach, read_inbox, read_now};
use super::requests::{self, AnswerOutcome, Request};
use super::{Ask, FeedStatus, MissionMachine, SessionNow};
use crate::fleet::link::{Connection, Post, now_ms};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::Session;
use crate::{Rail, State};
use rune::Tx;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// How often each machine is asked what its sessions are doing; the sessions themselves come with the feed's events.
pub const NOW_EVERY: f64 = 4.0;

/// One line when a feed's status changes (not one per read), as the machine reads do; `FLEET_NATIVE_MISSION_LOG` also says
/// each read, with the wall clock in ms, for checking a live run.
fn say_status(name: &str, before: FeedStatus, after: FeedStatus) {
    if before != after {
        eprintln!("fleet-native: mission control: {name}: {before:?} -> {after:?}");
    }
}

fn say_event(key: &str) {
    if std::env::var_os("FLEET_NATIVE_MISSION_LOG").is_some() {
        eprintln!("fleet-native: mission control: {key}: sessions event at {} ms", now_ms() % 1_000_000);
    }
}

fn say_read(name: &str, n: u32) {
    if std::env::var_os("FLEET_NATIVE_MISSION_LOG").is_some() {
        eprintln!("fleet-native: mission control: {name}: read {n} at {} ms", now_ms() % 1_000_000);
    }
}

/// What a link's thread noted for the next poll.
#[derive(Debug)]
enum Note {
    /// A machine's hub came up (true) or went down.
    Hub(String, bool),
    /// A machine's `sessions` topic said something changed.
    Sessions(String),
}

/// One open feed: the machine it reads, where, and what it last brought.
struct Open {
    id: u64,
    key: String,
    name: String,
    /// The address and token it was opened with: a feed whose machine moved or took a new token is replaced.
    base: String,
    token: Option<String>,
    feed: Feed,
    inbox: Inbox,
    now: HashMap<String, SessionNow>,
    /// When the now lines are next asked for.
    now_at: f64,
    reads: u32,
}

/// The open feeds, in the machines list's order.
#[derive(Default)]
pub struct Mission {
    feeds: Vec<Open>,
    notes: Vec<Note>,
    next_id: u64,
    /// Machines a scene or a test holds in place of feeds (a scene has no Fleet to read).
    pub(crate) fixed: Option<Vec<MissionMachine>>,
    /// The tiles whose answer is on its way (`NeedsYouColumn`'s `busy`).
    pub(crate) busy: HashSet<String>,
    /// What went wrong with a tile's last answer, by tile key; a good answer takes it away.
    pub(crate) errors: HashMap<String, String>,
    /// How a machine's view groups its tiles; one choice for every machine, kept while the app runs and not saved.
    pub grouping: super::Grouping,
    /// The tile key a press picked (`selectedKey`); the peek shows it while that tile is on the open machine's page.
    pub picked: Option<String>,
    /// The peek's reads, field and outcome.
    pub peek: super::peeking::Peek,
}

impl Mission {
    /// The keys of the machines whose feed is open, in order.
    pub fn open_keys(&self) -> Vec<&str> {
        self.feeds.iter().map(|f| f.key.as_str()).collect()
    }

    pub fn is_open(&self) -> bool {
        !self.feeds.is_empty()
    }

    /// How a machine's feed is doing; none when it has no feed.
    pub fn status(&self, key: &str) -> Option<FeedStatus> {
        self.find(key).map(|f| f.feed.status)
    }

    /// Why the machine's last read failed, when it said.
    pub fn error(&self, key: &str) -> Option<&str> {
        self.find(key).and_then(|f| f.feed.error.as_deref())
    }

    /// How many reads of its sessions the machine's feed has started.
    pub fn reads(&self, key: &str) -> u32 {
        self.find(key).map_or(0, |f| f.reads)
    }

    /// A copy of the listed session `id` of machine `key`, as that machine last read it.
    pub fn session(&self, key: &str, id: &str) -> Option<Session> {
        if let Some(fixed) = &self.fixed {
            return fixed.iter().find(|m| m.id == key)?.sessions.iter().find(|s| s.id == id).map(Session::row_copy);
        }
        self.find(key)?.inbox.sessions.iter().find(|s| s.id == id).map(Session::row_copy)
    }

    fn find(&self, key: &str) -> Option<&Open> {
        self.feeds.iter().find(|f| f.key == key)
    }

    /// The machines as mission control holds them: what each feed last read, with its status.
    pub fn machines(&self) -> Vec<MissionMachine> {
        if let Some(fixed) = &self.fixed {
            return fixed.clone();
        }
        self.feeds
            .iter()
            .map(|f| MissionMachine {
                id: f.key.clone(),
                name: f.name.clone(),
                status: f.feed.status,
                last_heard_at: f.feed.last_heard_at,
                sessions: f.inbox.sessions.iter().map(Session::row_copy).collect(),
                asks: f.inbox.asks.clone(),
                now: f.now.clone(),
            })
            .collect()
    }
}

impl State {
    /// Whether mission control's feeds are wanted: the Fleet view is shown, and on, and there is a Fleet to ask.
    fn mission_wanted(&self) -> bool {
        self.link.is_some() && self.rail == Rail::Fleet && self.labs_feature(crate::fleet::labs::Feature::Fleet) == Some(true)
    }

    /// The link's thread says a machine's hub came up or went down.
    pub fn mission_hub(&mut self, key: &str, connection: &Connection) {
        match connection {
            Connection::Online => self.mission.notes.push(Note::Hub(key.to_string(), true)),
            Connection::Down(_) | Connection::Refused => self.mission.notes.push(Note::Hub(key.to_string(), false)),
            Connection::Connecting => {}
        }
    }

    /// The link's thread says a machine's `sessions` topic changed.
    pub fn mission_sessions_event(&mut self, key: &str) {
        say_event(key);
        self.mission.notes.push(Note::Sessions(key.to_string()));
    }

    /// Where a call to the machine `key` goes: home's own server, or the machine at its address.
    pub fn mission_reach(&self, key: &str) -> Option<Reach> {
        if key == HOME_MACHINE_KEY {
            return self.link.as_ref().map(|l| Reach::Home(l.base.clone()));
        }
        self.machines.machine(key).map(|m| Reach::Machine(Box::new(m.clone())))
    }

    /// Opens, syncs and closes the feeds as the Fleet view is shown and left, and reads when a feed's time comes. True when
    /// something changed.
    pub(crate) fn keep_mission(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.mission_wanted() {
            return self.mission_close_all();
        }
        let mut changed = self.mission_sync(now);
        changed |= self.keep_peek();
        for note in std::mem::take(&mut self.mission.notes) {
            let (Note::Hub(key, _) | Note::Sessions(key)) = &note;
            let Some(open) = self.mission.feeds.iter_mut().find(|f| f.key == *key) else { continue };
            let before = open.feed.status;
            match note {
                Note::Hub(_, true) => {
                    open.feed.hub_connected();
                }
                Note::Hub(_, false) => open.feed.hub_lost(now),
                Note::Sessions(_) => open.feed.event(now),
            }
            say_status(&open.name, before, open.feed.status);
            changed = true;
        }
        let visible = self.quiet.visible;
        let mut wake = f64::INFINITY;
        for at in 0..self.mission.feeds.len() {
            if self.mission.feeds[at].feed.due(now) {
                self.mission_read(at);
            }
            let open = &mut self.mission.feeds[at];
            // Fleet asks nothing while its document is hidden, and asks at once when it shows again.
            if visible && now >= open.now_at {
                open.now_at = now + NOW_EVERY;
                self.mission_now(at);
            }
            let open = &self.mission.feeds[at];
            wake = wake.min(open.feed.next_deadline().unwrap_or(f64::INFINITY)).min(open.now_at);
        }
        if wake.is_finite() {
            tx.wake_after(Duration::from_secs_f64((wake - now).clamp(0.02, 1.0)));
        }
        changed
    }

    /// Leaving the view closes every feed: each stops, lets go of the machine's `sessions` topic and takes its reads with it.
    fn mission_close_all(&mut self) -> bool {
        self.mission.notes.clear();
        if self.mission.feeds.is_empty() {
            return false;
        }
        for mut open in std::mem::take(&mut self.mission.feeds) {
            open.feed.stop();
            self.mission_listen(&open.key, false);
        }
        true
    }

    /// Tells the link of machine `key` whether its `sessions` topic is wanted. A machine with no link yet has nothing to tell.
    fn mission_listen(&self, key: &str, on: bool) {
        let link = if on { self.link_for_machine(key) } else { self.link_if_any(key) };
        if let Some(link) = link {
            link.feed(on);
        }
    }

    /// Opens a feed for each machine that has none, and closes those of machines that are gone or changed (`sync`): a
    /// feed is the same while its key, address and token are; a rename alone keeps it.
    fn mission_sync(&mut self, now: f64) -> bool {
        let wanted: Vec<(String, String, String, Option<String>)> = self
            .machines
            .entries()
            .into_iter()
            .filter_map(|e| match e.machine {
                Some(m) => Some((e.key, e.name, m.base().to_string(), m.token.as_ref().map(|t| t.expose().to_string()))),
                None => Some((e.key, e.name, self.link.as_ref()?.base.clone(), None)),
            })
            .collect();
        let same = |open: &Open, w: &(String, String, String, Option<String>)| open.key == w.0 && open.base == w.2 && open.token == w.3;
        let mut changed = false;
        let mut old = std::mem::take(&mut self.mission.feeds);
        let mut kept: Vec<Open> = Vec::new();
        for w in &wanted {
            match old.iter().position(|o| same(o, w)) {
                Some(at) => {
                    let mut open = old.remove(at);
                    changed |= std::mem::replace(&mut open.name, w.1.clone()) != w.1;
                    kept.push(open);
                }
                None => {
                    self.mission.next_id += 1;
                    let id = self.mission.next_id;
                    kept.push(Open {
                        id,
                        key: w.0.clone(),
                        name: w.1.clone(),
                        base: w.2.clone(),
                        token: w.3.clone(),
                        feed: Feed::new(),
                        inbox: Inbox::default(),
                        now: HashMap::new(),
                        now_at: now,
                        reads: 0,
                    });
                    changed = true;
                }
            }
        }
        for mut gone in old {
            gone.feed.stop();
            if !wanted.iter().any(|w| w.0 == gone.key) {
                self.mission_listen(&gone.key, false);
            }
            changed = true;
        }
        self.mission.feeds = kept;
        for at in 0..self.mission.feeds.len() {
            if self.mission.feeds[at].feed.is_stopped() {
                self.mission_start(at, now);
            }
        }
        changed
    }

    /// Starts a feed that was just opened: its link listens to `sessions`, and its first read goes out.
    fn mission_start(&mut self, at: usize, now: f64) {
        let key = self.mission.feeds[at].key.clone();
        self.mission_listen(&key, true);
        let up = if key == HOME_MACHINE_KEY {
            self.connection == Connection::Online
        } else {
            self.machines.hubs.get(&key) == Some(&Connection::Online)
        };
        let down = if key == HOME_MACHINE_KEY {
            matches!(self.connection, Connection::Down(_) | Connection::Refused)
        } else {
            matches!(self.machines.hubs.get(&key), Some(Connection::Down(_) | Connection::Refused))
        };
        let open = &mut self.mission.feeds[at];
        let read = open.feed.start(now);
        if up {
            open.feed.hub_connected();
        } else if down {
            open.feed.hub_lost(now);
        }
        say_status(&open.name, FeedStatus::Connecting, open.feed.status);
        if read {
            self.mission_read(at);
        }
    }

    fn mission_post(&self) -> Option<Post> {
        self.link.as_ref().map(|l| l.post())
    }

    /// Reads the machine of feed `at`: its sessions, then what the waiting ones wait on.
    fn mission_read(&mut self, at: usize) {
        let (Some(post), Some(reach)) = (self.mission_post(), self.mission_reach(&self.mission.feeds[at].key)) else { return };
        let open = &mut self.mission.feeds[at];
        open.reads += 1;
        say_read(&open.name, open.reads);
        let id = open.id;
        std::thread::spawn(move || {
            let result = read_inbox(&reach, now_ms());
            post.update(move |s, tx| s.mission_read_done(id, result, tx));
        });
    }

    fn mission_read_done(&mut self, id: u64, result: Result<Inbox, Option<String>>, tx: &mut Tx) {
        let Some(at) = self.mission.feeds.iter().position(|f| f.id == id) else { return };
        let open = &mut self.mission.feeds[at];
        let outcome = match result {
            Ok(inbox) => {
                open.inbox = inbox;
                Ok(now_ms())
            }
            // A failed read keeps the last sessions and asks, and says why.
            Err(why) => Err(why),
        };
        let before = open.feed.status;
        let again = open.feed.read_done(outcome);
        say_status(&open.name, before, open.feed.status);
        if again {
            self.mission_read(at);
        }
        tx.wake_after(Duration::from_millis(20));
    }

    /// Asks the machine of feed `at` what each of its sessions is doing.
    fn mission_now(&mut self, at: usize) {
        let (Some(post), Some(reach)) = (self.mission_post(), self.mission_reach(&self.mission.feeds[at].key)) else { return };
        let id = self.mission.feeds[at].id;
        std::thread::spawn(move || {
            let lines = read_now(&reach);
            post.update(move |s, tx| {
                if let Some(open) = s.mission.feeds.iter_mut().find(|f| f.id == id) {
                    open.now = lines;
                }
                tx.wake_after(Duration::from_millis(20));
            });
        });
    }

    /// Reads the machine `key` at once, after an answer or a nudge: the card goes when the machine says the ask is gone.
    pub fn mission_refresh(&mut self, key: &str) {
        let Some(at) = self.mission.feeds.iter().position(|f| f.key == key) else { return };
        if self.mission.feeds[at].feed.refresh() {
            self.mission_read(at);
        }
    }

    /// Sends an answer or a nudge to the machine `key` on a thread; `done` runs on the UI thread with how it went, after the
    /// machine is read again. A machine that isn't in the list says so without sending anything.
    pub fn mission_send(&mut self, key: &str, request: Request, done: impl FnOnce(&mut State, AnswerOutcome) + Send + 'static) {
        let (Some(post), Some(reach)) = (self.mission_post(), self.mission_reach(key).filter(|_| self.mission.find(key).is_some())) else {
            let gone = AnswerOutcome { ok: false, gone: false, error: Some(requests::MACHINE_GONE.to_string()) };
            return done(self, gone);
        };
        let key = key.to_string();
        std::thread::spawn(move || {
            let sent = request.send(&reach.base());
            let outcome = requests::outcome(&sent);
            post.update(move |s, _| {
                s.mission_refresh(&key);
                done(s, outcome);
            });
        });
    }

    /// Whether the harness of a session on machine `key` takes a message into the running turn, from that machine's own
    /// `GET /api/harnesses`; a failed read says no. `done` runs on the UI thread.
    pub fn mission_steers(&self, key: &str, harness_type: &str, done: impl FnOnce(&mut State, bool) + Send + 'static) {
        let (Some(post), Some(reach)) = (self.mission_post(), self.mission_reach(key)) else { return };
        let harness = harness_type.to_string();
        std::thread::spawn(move || {
            let body: Option<Value> =
                reach.get("/api/harnesses").ok().filter(|(s, _)| (200..300).contains(s)).and_then(|(_, b)| serde_json::from_str(&b).ok());
            let steers = requests::steers(body.as_ref(), &harness);
            post.update(move |s, _| done(s, steers));
        });
    }

    /// The asks of a machine's waiting sessions, as its feed last read them.
    pub fn mission_ask(&self, key: &str, session: &str) -> Option<&Ask> {
        self.mission.find(key)?.inbox.asks.get(session)
    }
}
