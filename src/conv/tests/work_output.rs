//! `WorkOutput.vue`'s reading (rows 4.29, 4.30) as new tests: the tail clip, the 8-page cap, the first jump to the tail
//! of a long output, a read that stops at the end, and what an error leaves.
use crate::conv::work_output::*;

fn page(output: &str, next_offset: u64, size: u64) -> Result<Page, String> {
    Ok(Page { output: output.into(), next_offset, size, truncated: false })
}

#[test]
fn a_short_output_is_read_whole_in_one_page() {
    let mut r = Reader::new(1, true);
    assert_eq!(r.begin(), 0);
    assert_eq!(r.page(page("✓ 12 tests passed\n", 19, 19)), Next::Done);
    assert_eq!(r.text, "✓ 12 tests passed\n");
    assert!(!r.clipped && !r.loading && !r.in_flight && r.error.is_none());
}

#[test]
fn a_long_output_starts_at_its_tail_and_keeps_the_part_line_it_lands_in() {
    let mut r = Reader::new(1, true);
    r.begin();
    let size = 100_000;
    // The first answer only says how big it is: skip to the last 32 KiB and ask again.
    assert_eq!(r.page(page("start of it\n", 12, size)), Next::Fetch(size - TAIL_BYTES));
    assert!(r.clipped && r.text.is_empty());
    assert_eq!(r.page(page("rt of a line\nfull line\n", size, size)), Next::Done);
    // Fleet sets `started` at the jump, so its "drop the part line" branch never runs; the port does the same, and the part line stays.
    assert_eq!(r.text, "rt of a line\nfull line\n");
}

#[test]
fn it_keeps_only_the_last_64k_characters_and_says_it_dropped_some() {
    let mut r = Reader::new(1, false);
    r.begin();
    let chunk = "é".repeat(TAIL_CHARS);
    r.page(page(&chunk, 10, 10));
    assert!(!r.clipped);
    r.begin();
    r.page(page("tail", 14, 14));
    assert!(r.clipped);
    assert_eq!(r.text.chars().count(), TAIL_CHARS);
    assert!(r.text.ends_with("éétail"));
}

#[test]
fn it_reads_at_most_8_pages_a_go_and_carries_on_from_its_offset_on_the_next() {
    let mut r = Reader::new(1, true);
    r.begin();
    let mut asked = vec![];
    let mut at = 0;
    for _ in 0..MAX_PAGES - 1 {
        at += 10;
        match r.page(page("0123456789", at, 1_000)) {
            Next::Fetch(o) => asked.push(o),
            Next::Done => panic!("stopped early"),
        }
    }
    assert_eq!(asked.last(), Some(&70));
    assert_eq!(r.page(page("0123456789", 80, 1_000)), Next::Done, "the eighth page ends the go");
    assert_eq!(r.text.len(), 80);
    assert_eq!(r.begin(), 80, "the next go starts where this one stopped");
}

#[test]
fn an_empty_page_ends_the_read_even_when_the_size_says_more() {
    let mut r = Reader::new(1, true);
    r.begin();
    assert_eq!(r.page(page("", 0, 50)), Next::Done);
}

#[test]
fn the_server_saying_it_dropped_output_marks_it_clipped() {
    let mut r = Reader::new(1, true);
    r.begin();
    r.page(Ok(Page { output: "x".into(), next_offset: 1, size: 1, truncated: true }));
    assert!(r.clipped);
}

#[test]
fn a_failed_read_is_a_note_only_while_nothing_was_read() {
    let mut r = Reader::new(1, true);
    r.begin();
    assert_eq!(r.page(Err("The output couldn't be read (HTTP 404).".into())), Next::Done);
    assert_eq!(r.error.as_deref(), Some("The output couldn't be read (HTTP 404)."));
    r.begin();
    r.page(page("got it\n", 7, 7));
    assert!(r.error.is_none());
    r.begin();
    r.page(Err("gone".into()));
    assert!(r.error.is_none(), "what was read stays and the error is swallowed");
    assert_eq!(r.text, "got it\n");
}
