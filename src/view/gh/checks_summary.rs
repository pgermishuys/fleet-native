//! `ChecksSummary.vue`: GitHub's words for a pull request's checks, the counts, and a bar split into passed, failing and
//! running.
use super::check_icon::{CheckKind, check_icon};
use crate::fleet::pr_state::{CheckCounts, Checks, check_count_words, checks_from_counts, checks_headline};
use crate::theme::{Theme, sans};
use rune::{El, col, el, row, text};

/// `bar_only` leaves out the headline and the counts, for rows that only need the bar.
pub fn checks_summary(t: &Theme, key: &str, counts: CheckCounts, bar_only: bool) -> El {
    let state = checks_from_counts(counts);
    let total = counts.passed + counts.failing + counts.pending;
    let mut summary = col().key(key.to_string()).w_full().min_w(0.0).gap(8.0);
    if !bar_only {
        let icon = match state {
            Checks::Failing => CheckKind::Failure,
            Checks::Pending => CheckKind::Pending,
            _ => CheckKind::Success,
        };
        summary = summary.child(
            row()
                .w_full()
                .flex_wrap()
                .center_y()
                .gap(8.0)
                .child(check_icon(t, icon, 14.0))
                .child(text(checks_headline(state)).font(sans(12.5, 500.0, 1.3)).color(t.text))
                // `margin-left: auto`: right-aligned, and onto a line of its own when the headline leaves no room.
                .child(
                    row()
                        .grow(1.0)
                        .justify_end()
                        .child(text(check_count_words(counts)).font(sans(11.5, 400.0, 1.3).features("'tnum' 1")).color(t.muted)),
                ),
        );
    }
    if total > 0 {
        let segment = |n: u32, color| (n > 0).then(|| el().grow(n as f32).h(5.0).fill(color));
        let mut bar = row()
            .key(format!("{key}-bar"))
            .w_full()
            .h(5.0)
            .gap(2.0)
            .radius(999.0)
            .clip_rounded(999.0)
            .fill(t.wash(8.0))
            .label(check_count_words(counts));
        for part in
            [segment(counts.passed, t.check_pass()), segment(counts.failing, t.check_fail()), segment(counts.pending, t.check_pending())]
                .into_iter()
                .flatten()
        {
            bar = bar.child(part);
        }
        summary = summary.child(bar);
    }
    summary
}
