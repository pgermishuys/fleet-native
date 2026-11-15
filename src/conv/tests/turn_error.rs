//! `lib/__tests__/turn-error.test.ts`, under the same names.
use crate::conv::turn_error::split_turn_error_message;

#[test]
fn keeps_the_sentence_and_folds_the_stack_trace_open_code_appends() {
    let message = "ProviderModelNotFoundError: Model not found: github-copilot/claude-sonnet-4.5. Did you mean: claude-sonnet-5? \
        at <anonymous> (/$bunfs/root/chunk-y568bp57.js:439:94278) at SessionPrompt.getModel (/$bunfs/root/chunk-9mk16qmt.js:1142:11505)";
    let got = split_turn_error_message(message);
    assert_eq!(got.summary, "Model not found: github-copilot/claude-sonnet-4.5. Did you mean: claude-sonnet-5?");
    assert_eq!(
        got.details.as_deref(),
        Some(
            "at <anonymous> (/$bunfs/root/chunk-y568bp57.js:439:94278) at SessionPrompt.getModel (/$bunfs/root/chunk-9mk16qmt.js:1142:11505)"
        )
    );
}

#[test]
fn leaves_a_message_without_a_stack_trace_as_it_is() {
    let got = split_turn_error_message("Rate limit reached, try again in 20 seconds");
    assert_eq!((got.summary.as_str(), got.details), ("Rate limit reached, try again in 20 seconds", None));
}

#[test]
fn keeps_an_error_name_that_is_the_whole_message() {
    let got = split_turn_error_message("AbortError: ");
    assert_eq!((got.summary.as_str(), got.details), ("AbortError:", None));
}

#[test]
fn finds_multi_line_stack_traces() {
    let got = split_turn_error_message("TypeError: x is undefined\n    at run (file:///a.js:1:2)\n    at main (file:///a.js:3:4)");
    assert_eq!(got.summary, "x is undefined");
    assert_eq!(got.details.as_deref(), Some("at run (file:///a.js:1:2)\n    at main (file:///a.js:3:4)"));
}

#[test]
fn links_the_web_addresses_in_the_paragraph_and_not_the_full_stop_after_them() {
    use crate::conv::turn_error::url_ranges;
    let text = "Billing is at https://example.test/plan?x=1. Docs (https://example.test/docs) and ftp://nope.test too.";
    let found: Vec<&str> = url_ranges(text).into_iter().map(|r| &text[r]).collect();
    assert_eq!(found, ["https://example.test/plan?x=1", "https://example.test/docs"]);
    assert!(url_ranges("http is a protocol").is_empty());
}
