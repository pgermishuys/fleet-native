//! `lib/__tests__/automation-schedule.test.ts` under its names, with Fleet's NOW (Tuesday 15 September 2026, 10:00) on a
//! clock at UTC, and then the same logic across a daylight-saving change and in a half-hour zone, which Fleet's own
//! tests can't pin down because they run on whatever clock the machine has.
use super::super::automations::schedule::*;
use crate::conv::time::DAY;

fn utc(_: i64) -> i64 {
    0
}

/// 2026 in America/New_York: summer time from 8 March 07:00 UTC (02:00 local, skipped to 03:00) to 1 November 06:00 UTC
/// (02:00 local, 01:00-02:00 repeated).
fn new_york(ms: i64) -> i64 {
    let (start, end) = (ms_of(2026, 3, 8, 7, 0), ms_of(2026, 11, 1, 6, 0));
    if ms >= start && ms < end { -4 * 3600 } else { -5 * 3600 }
}

fn kolkata(_: i64) -> i64 {
    5 * 3600 + 1800
}

/// Unix ms of a UTC date and time (month 1-12).
fn ms_of(y: i64, mo: i64, d: i64, h: i64, mi: i64) -> i64 {
    crate::fleet::automations::schedule::days_from_civil(y, mo - 1, d) * DAY + (h * 60 + mi) * 60_000
}

fn now() -> i64 {
    ms_of(2026, 9, 15, 10, 0)
}

fn when_of(text: &str) -> Option<When> {
    parse_schedule(text).map(|h| h.when)
}

const T9: Hm = (9, 0);

fn weekly(days: &[u8], t: Hm) -> When {
    When::Weekly { days: days.to_vec(), t, ambiguous: false }
}
fn once_day(day: u8, t: Hm) -> When {
    When::Once { t, day: Some(day), rel: None, date: None }
}

// ─── parseSchedule ──────────────────────────────────────────────────────────

#[test]
fn parse_schedule_reads_each_sentence() {
    let cases: Vec<(&str, When)> = vec![
        ("Every Monday at 9am, summarise the open PRs", weekly(&[1], T9)),
        ("Weekdays at 8:30, list yesterday's failed CI runs", weekly(&[1, 2, 3, 4, 5], (8, 30))),
        ("Every 2 hours, check whether the nightly E2E run is flaky", When::Hours { n: 2 }),
        ("Every 15 minutes, ping the preview", When::Minutes { n: 15 }),
        ("Hourly, look at the queue", When::Hours { n: 1 }),
        ("On the 1st of every month at 10, prune worktrees", When::Monthly { dom: 1, t: (10, 0) }),
        ("Every Tuesday and Thursday at 4pm, triage issues", weekly(&[2, 4], (16, 0))),
        ("Every day at noon, post a summary", weekly(&[0, 1, 2, 3, 4, 5, 6], (12, 0))),
        ("Nightly, check for outdated packages", weekly(&[0, 1, 2, 3, 4, 5, 6], (21, 0))),
        ("Mondays at 7:45, plan the week", weekly(&[1], (7, 45))),
        ("Create an automation that runs once on a monday to check the release notes", once_day(1, T9)),
        ("Tomorrow at 3pm, remind me to review #212", When::Once { t: (15, 0), day: None, rel: Some(Rel::Tomorrow), date: None }),
        ("Next Friday, tidy the changelog", once_day(5, T9)),
        ("Tidy the changelog on Friday.", When::Weekly { days: vec![5], t: T9, ambiguous: true }),
        ("Tidy the changelog on a Friday.", weekly(&[5], T9)),
    ];
    for (text, expected) in cases {
        assert_eq!(when_of(text), Some(expected), "reads {text:?}");
    }
}

#[test]
fn parse_schedule_finds_nothing_in_a_sentence_without_a_schedule() {
    assert_eq!(parse_schedule("Summarise the open PRs"), None);
}

#[test]
fn parse_schedule_says_where_the_schedule_words_are_so_they_can_be_highlighted() {
    let text = "Please, every Monday at 9, summarise the open PRs";
    let hit = parse_schedule(text).unwrap();
    assert_eq!(&text[hit.start..hit.end], "every Monday at 9");
}

#[test]
fn parse_schedule_does_not_take_a_possessive_for_a_day() {
    // The look-ahead `(?![\w'])`: "Monday's" is a noun, not "on Monday".
    assert_eq!(parse_schedule("Summarise on Monday's notes"), None);
    assert_eq!(when_of("Summarise on Monday, notes"), Some(When::Weekly { days: vec![1], t: T9, ambiguous: true }));
    // A day that runs into a longer word is not a day either.
    assert_eq!(parse_schedule("Fix it on Mondaynight"), None);
}

#[test]
fn parse_schedule_reads_times_and_parts_of_the_day() {
    assert_eq!(when_of("every day in the evening"), Some(weekly(&[0, 1, 2, 3, 4, 5, 6], (18, 0))));
    assert_eq!(when_of("every morning"), Some(weekly(&[0, 1, 2, 3, 4, 5, 6], T9)));
    assert_eq!(when_of("every friday afternoon"), Some(weekly(&[5], (14, 0))));
    assert_eq!(when_of("every friday at 12am"), Some(weekly(&[5], (0, 0))));
    assert_eq!(when_of("every friday at 12 pm"), Some(weekly(&[5], (12, 0))));
    assert_eq!(when_of("every friday, midnight"), Some(weekly(&[5], (0, 0))));
    // Out of range is no time: the part of the day, else 09:00.
    assert_eq!(when_of("every friday at 25"), Some(weekly(&[5], T9)));
    assert_eq!(when_of("every friday evening at 13:75"), Some(weekly(&[5], (18, 0))));
    assert_eq!(when_of("monthly on the 31st"), Some(When::Monthly { dom: 1, t: T9 }));
    assert_eq!(when_of("on the 31st of every month"), Some(When::Monthly { dom: 28, t: T9 }));
}

// ─── promptFrom ─────────────────────────────────────────────────────────────

fn prompt(text: &str) -> String {
    prompt_from(text, parse_schedule(text).as_ref())
}

#[test]
fn prompt_from_leaves_out_the_schedule_words() {
    assert_eq!(
        prompt("Every Monday at 9am, go through the open pull requests and summarise each one."),
        "Go through the open pull requests and summarise each one."
    );
}

#[test]
fn prompt_from_leaves_out_create_an_automation_that_which_is_meant_for_fleet() {
    assert_eq!(
        prompt("Create an automation that runs once on a monday to check whether the v0.24 release notes cover everything."),
        "Check whether the v0.24 release notes cover everything."
    );
}

#[test]
fn prompt_from_leaves_a_prompt_with_nothing_to_take_out_as_it_was_written() {
    assert_eq!(prompt_from("summarise the open PRs:", None), "summarise the open PRs:");
    assert_eq!(prompt_from("  summarise the open PRs  ", None), "summarise the open PRs");
}

#[test]
fn prompt_from_keeps_line_breaks_in_a_longer_prompt() {
    assert_eq!(prompt("Every day at 9, check CI.\nThen post the result."), "Check CI.\nThen post the result.");
}

// ─── autoName ───────────────────────────────────────────────────────────────

#[test]
fn auto_name_takes_the_first_words_of_the_first_sentence() {
    assert_eq!(auto_name("Go through the open pull requests in weave-fleet and summarise each one."), "Go through the open pull requests");
    assert_eq!(auto_name(""), "");
    assert_eq!(auto_name("Check CI, then post."), "Check CI");
}

#[test]
fn auto_name_cuts_a_long_name_at_forty_characters_and_adds_an_ellipsis() {
    let name = auto_name("Reconfigure internationalisation infrastructure documentation consolidation");
    assert!(name.ends_with('…'), "{name}");
    assert!(name.chars().count() <= 41, "{name}");
}

// ─── nextRun ────────────────────────────────────────────────────────────────

#[test]
fn next_run_finds_the_next_matching_day_and_time() {
    let n = now();
    assert_eq!(next_run(&weekly(&[1], T9), n, &utc), Some(ms_of(2026, 9, 21, 9, 0)));
    assert_eq!(next_run(&weekly(&[2], (11, 0)), n, &utc), Some(ms_of(2026, 9, 15, 11, 0)));
    assert_eq!(next_run(&When::Hours { n: 2 }, n, &utc), Some(ms_of(2026, 9, 15, 12, 0)));
    assert_eq!(next_run(&When::Monthly { dom: 1, t: (10, 0) }, n, &utc), Some(ms_of(2026, 10, 1, 10, 0)));
    assert_eq!(next_run(&When::Minutes { n: 15 }, n, &utc), Some(ms_of(2026, 9, 15, 10, 15)));
    // Today's time has passed, so a weekly run for today waits a week.
    assert_eq!(next_run(&weekly(&[2], (10, 0)), n, &utc), Some(ms_of(2026, 9, 22, 10, 0)));
    assert_eq!(next_run(&When::Cron { expr: "0 9 1-7 * 1".into() }, n, &utc), None);
    assert_eq!(next_run(&When::Event { event_type: "session_created".into() }, n, &utc), None);
    assert_eq!(next_run(&When::Hours { n: 0 }, n, &utc), None);
}

#[test]
fn next_run_puts_a_one_off_on_the_next_such_day_or_the_date_it_was_fixed_to() {
    let n = now();
    assert_eq!(next_run(&once_day(1, T9), n, &utc), Some(ms_of(2026, 9, 21, 9, 0)));
    let tomorrow = When::Once { t: (15, 0), day: None, rel: Some(Rel::Tomorrow), date: None };
    assert_eq!(next_run(&tomorrow, n, &utc), Some(ms_of(2026, 9, 16, 15, 0)));
    let fixed = When::Once { t: (8, 30), day: None, rel: None, date: Some("2026-10-02".into()) };
    assert_eq!(next_run(&fixed, n, &utc), Some(ms_of(2026, 10, 2, 8, 30)));
    let today = |t| When::Once { t, day: None, rel: Some(Rel::Today), date: None };
    assert_eq!(next_run(&today((11, 0)), n, &utc), Some(ms_of(2026, 9, 15, 11, 0)));
    assert_eq!(next_run(&today((9, 0)), n, &utc), Some(ms_of(2026, 9, 16, 9, 0)));
    // The day named is today and its time has passed: next week.
    assert_eq!(next_run(&once_day(2, (9, 0)), n, &utc), Some(ms_of(2026, 9, 22, 9, 0)));
}

// ─── the local clock: daylight saving and a half-hour zone ──────────────────

#[test]
fn next_run_keeps_the_local_hour_when_the_offset_changes_before_the_run() {
    // Saturday 7 March 12:00 in New York (EST, UTC-5). Sunday 09:00 is after the change to EDT (UTC-4): 13:00 UTC.
    let from = ms_of(2026, 3, 7, 17, 0);
    assert_eq!(next_run(&weekly(&[0], T9), from, &new_york), Some(ms_of(2026, 3, 8, 13, 0)));
    // The same schedule in the week before the change is at 14:00 UTC.
    let earlier = ms_of(2026, 2, 28, 17, 0);
    assert_eq!(next_run(&weekly(&[0], T9), earlier, &new_york), Some(ms_of(2026, 3, 1, 14, 0)));
    // And the other way: Saturday 31 October 12:00 EDT, Sunday 09:00 is EST, 14:00 UTC.
    let autumn = ms_of(2026, 10, 31, 16, 0);
    assert_eq!(next_run(&weekly(&[0], T9), autumn, &new_york), Some(ms_of(2026, 11, 1, 14, 0)));
    // A monthly run on the 1st at 10:00 crosses the change.
    assert_eq!(next_run(&When::Monthly { dom: 1, t: (10, 0) }, ms_of(2026, 10, 15, 12, 0), &new_york), Some(ms_of(2026, 11, 1, 15, 0)));
}

#[test]
fn next_run_moves_a_time_the_spring_change_skips_forward_and_takes_the_first_of_a_repeated_one() {
    // 02:30 on 8 March doesn't exist in New York: 03:30 EDT, as Date does.
    let skipped = When::Once { t: (2, 30), day: None, rel: None, date: Some("2026-03-08".into()) };
    assert_eq!(next_run(&skipped, ms_of(2026, 3, 1, 0, 0), &new_york), Some(ms_of(2026, 3, 8, 7, 30)));
    // 01:30 on 1 November happens twice: the first, EDT.
    let repeated = When::Once { t: (1, 30), day: None, rel: None, date: Some("2026-11-01".into()) };
    assert_eq!(next_run(&repeated, ms_of(2026, 10, 1, 0, 0), &new_york), Some(ms_of(2026, 11, 1, 5, 30)));
}

#[test]
fn next_run_counts_hours_and_minutes_on_the_local_clock() {
    // Every 2 hours from 10:20 in Kolkata (UTC+5:30): the local hour 12 on the hour is 06:30 UTC.
    let from = ms_of(2026, 9, 15, 4, 50);
    assert_eq!(next_run(&When::Hours { n: 2 }, from, &kolkata), Some(ms_of(2026, 9, 15, 6, 30)));
    // Spring change: from 00:30 EST every 3 hours is 03:00 EDT (07:00 UTC), the local hour that follows the skipped one.
    let before = ms_of(2026, 3, 8, 5, 30);
    assert_eq!(next_run(&When::Hours { n: 3 }, before, &new_york), Some(ms_of(2026, 3, 8, 7, 0)));
    assert_eq!(next_run(&When::Minutes { n: 30 }, ms_of(2026, 9, 15, 4, 50), &kolkata), Some(ms_of(2026, 9, 15, 5, 0)));
}

#[test]
fn trigger_of_a_one_off_is_the_local_date_not_the_utc_one() {
    // 23:30 on Saturday 14 March in New York is 03:30 UTC on Sunday the 15th.
    let from = ms_of(2026, 3, 14, 12, 0);
    let once = once_day(6, (23, 30));
    assert_eq!(to_trigger(&once, from, &new_york).unwrap().trigger_config, "2026-03-14T23:30");
    assert_eq!(local_date(ms_of(2026, 3, 15, 3, 30), &new_york), "2026-03-14");
    assert_eq!(describe_date(ms_of(2026, 3, 15, 3, 30), &new_york), "Sat 14 Mar, 23:30");
}

// ─── triggers ───────────────────────────────────────────────────────────────

#[test]
fn triggers_store_each_when_as_the_trigger_the_server_reads_and_read_it_back() {
    let cases: Vec<(When, &str, &str)> = vec![
        (weekly(&[1], T9), "schedule", "0 9 * * 1"),
        (weekly(&[1, 2, 3, 4, 5], (8, 30)), "schedule", "30 8 * * 1-5"),
        (weekly(&[0, 1, 2, 3, 4, 5, 6], (21, 0)), "schedule", "0 21 * * *"),
        (weekly(&[2, 4], (16, 0)), "schedule", "0 16 * * 2,4"),
        (When::Hours { n: 1 }, "schedule", "0 * * * *"),
        (When::Hours { n: 2 }, "schedule", "0 */2 * * *"),
        (When::Minutes { n: 15 }, "schedule", "*/15 * * * *"),
        (When::Minutes { n: 1 }, "schedule", "* * * * *"),
        (When::Monthly { dom: 1, t: (10, 0) }, "schedule", "0 10 1 * *"),
        (once_day(1, T9), "once", "2026-09-21T09:00"),
        (When::Event { event_type: "session_created".into() }, "event", "{\"eventType\":\"session_created\"}"),
    ];
    for (when, trigger_type, trigger_config) in cases {
        let want = Trigger { trigger_type: trigger_type.into(), trigger_config: trigger_config.into() };
        assert_eq!(to_trigger(&when, now(), &utc), Some(want.clone()), "stores {when:?}");
        let back = from_trigger(trigger_type, trigger_config).unwrap_or_else(|| panic!("reads back {trigger_config}"));
        assert_eq!(to_trigger(&back, now(), &utc), Some(want), "reads back {when:?}");
    }
}

#[test]
fn triggers_keep_a_cron_it_has_no_words_for_as_custom() {
    assert_eq!(from_trigger("schedule", "0 9 1-7 * 1"), Some(When::Cron { expr: "0 9 1-7 * 1".into() }));
    assert_eq!(cron_of(&When::Cron { expr: "0 9 1-7 * 1".into() }), Some("0 9 1-7 * 1".into()));
}

#[test]
fn triggers_read_the_shapes_the_composer_writes_and_nothing_else_as_cron() {
    assert_eq!(from_trigger("schedule", "0 9 * * 1,3,3"), Some(weekly(&[1, 3], T9)));
    assert_eq!(from_trigger("schedule", "0 9 * * 3,1"), Some(weekly(&[1, 3], T9)));
    assert_eq!(from_trigger("schedule", "0 9 15 * *"), Some(When::Monthly { dom: 15, t: T9 }));
    assert_eq!(from_trigger("schedule", "0 9 * 6 *"), Some(When::Cron { expr: "0 9 * 6 *".into() }));
    assert_eq!(from_trigger("schedule", "0 9 * *"), Some(When::Cron { expr: "0 9 * *".into() }));
    assert_eq!(from_trigger("schedule", "0 9 * * * *"), Some(When::Cron { expr: "0 9 * * * *".into() }));
    assert_eq!(from_trigger("schedule", "0 9 * * 7"), Some(When::Cron { expr: "0 9 * * 7".into() }));
    assert_eq!(from_trigger("schedule", "  */5 * * * *  "), Some(When::Minutes { n: 5 }));
    assert_eq!(from_trigger("schedule", ""), Some(When::Cron { expr: String::new() }));
    assert_eq!(
        from_trigger("once", "2026-10-02T08:30"),
        Some(When::Once { t: (8, 30), day: None, rel: None, date: Some("2026-10-02".into()) })
    );
    assert_eq!(from_trigger("once", "tomorrow"), None);
    assert_eq!(from_trigger("event", "{\"eventType\":3}"), None);
    assert_eq!(from_trigger("event", "not json"), None);
    assert_eq!(from_trigger("webhook", "x"), None);
}

// ─── words ──────────────────────────────────────────────────────────────────

#[test]
fn words_label_the_when_chip_and_the_line_under_the_box() {
    let n = now();
    assert_eq!(when_chip(Some(&weekly(&[1], T9)), n, &utc), "Mondays 09:00");
    assert_eq!(when_chip(Some(&once_day(1, T9)), n, &utc), "Once · Mon 21 Sep");
    assert_eq!(when_chip(None, n, &utc), "When?");
    assert_eq!(when_sentence(&weekly(&[1, 2, 3, 4, 5], (8, 30)), n, &utc), "on weekdays at 08:30");
    assert_eq!(when_sentence(&once_day(1, T9), n, &utc), "once, on Mon 21 Sep at 09:00");
}

#[test]
fn words_cover_every_other_kind() {
    let n = now();
    let chip = |w: When| when_chip(Some(&w), n, &utc);
    assert_eq!(chip(weekly(&[0, 1, 2, 3, 4, 5, 6], (21, 0))), "Every day 21:00");
    assert_eq!(chip(weekly(&[1, 2, 3, 4, 5], T9)), "Weekdays 09:00");
    assert_eq!(chip(weekly(&[2, 4], (16, 0))), "Tue and Thu 16:00");
    assert_eq!(chip(weekly(&[1, 3, 5], T9)), "Mon, Wed and Fri 09:00");
    assert_eq!(chip(When::Hours { n: 1 }), "Every hour");
    assert_eq!(chip(When::Hours { n: 2 }), "Every 2 hours");
    assert_eq!(chip(When::Minutes { n: 1 }), "Every minute");
    assert_eq!(chip(When::Minutes { n: 15 }), "Every 15 min");
    assert_eq!(chip(When::Monthly { dom: 1, t: T9 }), "Monthly, 1st");
    assert_eq!(chip(When::Monthly { dom: 22, t: T9 }), "Monthly, 22nd");
    assert_eq!(chip(When::Monthly { dom: 11, t: T9 }), "Monthly, 11th");
    assert_eq!(chip(When::Monthly { dom: 13, t: T9 }), "Monthly, 13th");
    assert_eq!(chip(When::Cron { expr: "0 9 1-7 * 1".into() }), "0 9 1-7 * 1");
    assert_eq!(chip(When::Event { event_type: "x".into() }), "On an event");
    let sentence = |w: When| when_sentence(&w, n, &utc);
    assert_eq!(sentence(weekly(&[0, 1, 2, 3, 4, 5, 6], (21, 0))), "every day at 21:00");
    assert_eq!(sentence(weekly(&[1], T9)), "every Monday at 09:00");
    assert_eq!(sentence(When::Hours { n: 1 }), "every hour, on the hour");
    assert_eq!(sentence(When::Hours { n: 2 }), "every 2 hours, on the hour");
    assert_eq!(sentence(When::Minutes { n: 15 }), "every 15 minutes");
    assert_eq!(sentence(When::Monthly { dom: 1, t: (10, 0) }), "on the 1st of every month at 10:00");
    assert_eq!(sentence(When::Cron { expr: "0 9 1-7 * 1".into() }), "on the cron schedule 0 9 1-7 * 1");
    assert_eq!(sentence(When::Event { event_type: "x".into() }), "when the event happens");
    assert_eq!(day_name(5), "Friday");
}

#[test]
fn words_shorten_a_sidebar_rows_next_run_by_how_far_away_it_is() {
    let n = now();
    assert_eq!(next_short(ms_of(2026, 9, 15, 17, 0), n, &utc), "17:00");
    assert_eq!(next_short(ms_of(2026, 9, 21, 9, 0), n, &utc), "Mon 09:00");
    assert_eq!(next_short(ms_of(2026, 10, 1, 10, 0), n, &utc), "1 Oct");
    // Seven days away is "later", six is "this week".
    assert_eq!(next_short(ms_of(2026, 9, 21, 23, 0), n, &utc), "Mon 23:00");
    assert_eq!(next_short(ms_of(2026, 9, 22, 9, 0), n, &utc), "22 Sep");
    assert_eq!(describe_date(ms_of(2026, 9, 21, 9, 0), &utc), "Mon 21 Sep, 09:00");
}

#[test]
fn with_time_changes_the_time_and_stops_a_weekly_from_asking() {
    let asking = When::Weekly { days: vec![5], t: T9, ambiguous: true };
    assert_eq!(with_time(&asking, (7, 5)), weekly(&[5], (7, 5)));
    assert_eq!(with_time(&When::Hours { n: 2 }, (7, 5)), When::Hours { n: 2 });
    assert_eq!(with_time(&When::Monthly { dom: 3, t: T9 }, (7, 5)), When::Monthly { dom: 3, t: (7, 5) });
}

#[test]
fn local_offset_at_reads_the_zone_at_that_instant() {
    // Whatever the machine's zone, the offset is a whole number of minutes a day or less.
    let off = crate::conv::time::local_offset_at(ms_of(2026, 7, 1, 12, 0));
    assert!(off % 60 == 0 && off.abs() <= 14 * 3600, "{off}");
}
