//! Fleet's `issue-filter-query.test.ts` under its name, and tests for `filter-expression.ts` (Fleet has none) from its doc
//! comment: every qualifier, quoted values, an unclosed quote, unknown qualifiers kept, the round trip, defaults left out.
use super::filter::*;

fn own(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn turns_the_issue_filter_bars_choices_into_search_qualifiers() {
    let a = IssueFilter {
        labels: own(&["bug", "good first issue"]),
        assignee: Some("none".into()),
        author: Some("pat".into()),
        search: " flicker ".into(),
        ..DEFAULT
    };
    assert_eq!(issue_filter_query(&a), "is:open label:bug label:\"good first issue\" author:pat no:assignee sort:updated-desc flicker");
    let b = IssueFilter {
        state: StateChoice::All,
        milestone: Some("v0.36".into()),
        sort: SortField::Comments,
        direction: Direction::Asc,
        ..DEFAULT
    };
    assert_eq!(issue_filter_query(&b), "milestone:v0.36 sort:comments-asc");
}

#[test]
fn the_query_names_an_assignee_a_type_and_any_assignee() {
    let f = IssueFilter { assignee: Some("*".into()), kind: Some("Feature request".into()), ..DEFAULT };
    assert_eq!(issue_filter_query(&f), "is:open type:\"Feature request\" sort:updated-desc");
    let f = IssueFilter { assignee: Some("kit-lee".into()), ..DEFAULT };
    assert_eq!(issue_filter_query(&f), "is:open assignee:kit-lee sort:updated-desc");
}

#[test]
fn an_empty_expression_is_the_default() {
    assert_eq!(parse_filter_expression(""), DEFAULT);
    assert_eq!(parse_filter_expression("   \t "), DEFAULT);
    assert_eq!(serialize_filter_expression(&DEFAULT), "");
}

#[test]
fn every_qualifier_is_read() {
    let f = parse_filter_expression(
        "is:closed label:bug label:docs milestone:v1 assignee:kit-lee author:devon-park type:Bug sort:comments-asc crash",
    );
    assert_eq!(f.state, StateChoice::Closed);
    assert_eq!(f.labels, own(&["bug", "docs"]));
    assert_eq!(f.milestone.as_deref(), Some("v1"));
    assert_eq!(f.assignee.as_deref(), Some("kit-lee"));
    assert_eq!(f.author.as_deref(), Some("devon-park"));
    assert_eq!(f.kind.as_deref(), Some("Bug"));
    assert_eq!((f.sort, f.direction), (SortField::Comments, Direction::Asc));
    assert_eq!(f.search, "crash");
}

#[test]
fn the_special_assignee_and_type_values_are_kept() {
    let f = parse_filter_expression("assignee:none type:*");
    assert_eq!((f.assignee.as_deref(), f.kind.as_deref()), (Some("none"), Some("*")));
    assert_eq!(parse_filter_expression("assignee:*").assignee.as_deref(), Some("*"));
}

#[test]
fn quoted_values_keep_their_spaces() {
    let f = parse_filter_expression(r#"label:"good first issue" milestone:"v 0.36" two words"#);
    assert_eq!(f.labels, own(&["good first issue"]));
    assert_eq!(f.milestone.as_deref(), Some("v 0.36"));
    assert_eq!(f.search, "two words");
}

#[test]
fn an_unclosed_quote_takes_the_rest_of_the_line() {
    let f = parse_filter_expression(r#"is:open label:"good first"#);
    assert_eq!(f.labels, own(&["\"good first"]), "the quote stays: it isn't a pair");
    assert_eq!(f.search, "");
}

#[test]
fn unknown_qualifiers_and_unusable_values_stay_in_the_search_words() {
    let f = parse_filter_expression("is:merged repo:acme/rocket sort:fancy-desc sort:updated sort:created-sideways plain");
    assert_eq!(f.state, StateChoice::Open);
    assert_eq!(f.search, "is:merged repo:acme/rocket sort:fancy-desc sort:updated sort:created-sideways plain");
    assert_eq!((f.sort, f.direction), (SortField::Updated, Direction::Desc));
}

#[test]
fn a_label_is_not_repeated_and_an_empty_value_clears() {
    let f = parse_filter_expression("label:bug label:bug label: milestone: author:");
    assert_eq!(f.labels, own(&["bug"]));
    assert_eq!((f.milestone, f.author), (None, None));
}

#[test]
fn qualifier_names_are_not_case_sensitive_but_values_are() {
    let f = parse_filter_expression("IS:closed Label:Bug");
    assert_eq!((f.state, f.labels), (StateChoice::Closed, own(&["Bug"])));
}

#[test]
fn a_word_with_a_colon_that_is_not_a_qualifier_is_search_text() {
    assert_eq!(parse_filter_expression("12:30 launch").search, "12:30 launch");
    assert_eq!(parse_filter_expression("http://x.test").search, "http://x.test");
}

#[test]
fn defaults_are_left_out_and_the_search_goes_last() {
    let f = IssueFilter { search: "crash on launch".into(), labels: own(&["bug"]), ..DEFAULT };
    assert_eq!(serialize_filter_expression(&f), "label:bug crash on launch");
    let f = IssueFilter { state: StateChoice::All, sort: SortField::Created, direction: Direction::Asc, ..DEFAULT };
    assert_eq!(serialize_filter_expression(&f), "is:all sort:created-asc");
}

#[test]
fn a_value_with_a_space_a_quote_or_a_colon_is_quoted_and_escaped() {
    let f = IssueFilter { labels: own(&["good first issue", "a:b", "say \"hi\"", "plain"]), ..DEFAULT };
    assert_eq!(serialize_filter_expression(&f), r#"label:"good first issue" label:"a:b" label:"say \"hi\"" label:plain"#);
}

#[test]
fn what_is_typed_comes_back_as_typed() {
    for expr in [
        "",
        "is:closed",
        r#"label:bug label:"good first issue" milestone:"v 0.36" assignee:kit-lee author:devon-park type:Bug sort:created-asc crash"#,
        "assignee:none sort:comments-desc",
    ] {
        let once = parse_filter_expression(expr);
        assert_eq!(serialize_filter_expression(&once), expr, "{expr}");
        assert_eq!(parse_filter_expression(&serialize_filter_expression(&once)), once);
    }
}
