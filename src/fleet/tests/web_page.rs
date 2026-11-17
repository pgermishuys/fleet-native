//! Where a page canvas's page is (component 14, piece A): `server-canvas.test.ts`'s address case under its name, and the
//! address fields `shownPage` reads.
use super::super::web_page::*;
use serde_json::json;

fn page(page_id: &str, entry: &str) -> ShownPage {
    ShownPage { page_id: page_id.into(), entry: entry.into(), ..ShownPage::default() }
}

// server-canvas.test.ts: "addresses the page on Fleet, escaping each part of the file name"
#[test]
fn addresses_the_page_on_fleet_escaping_each_part_of_the_file_name() {
    assert_eq!(page_address(&page("pg_1", "options a.html")), "/pages/pg_1/options%20a.html");
    assert_eq!(page_address(&page("pg_1", "sub/b#c.html")), "/pages/pg_1/sub/b%23c.html");
}

#[test]
fn reads_the_fields_the_address_needs_and_leaves_the_rest_empty() {
    let read = shown_page(&json!({"pageId": "pg_2", "entry": "index.html", "source": "/a/pages/index.html", "shownAt": "t1", "files": 2}));
    assert_eq!(
        read,
        ShownPage { page_id: "pg_2".into(), entry: "index.html".into(), source: "/a/pages/index.html".into(), shown_at: "t1".into() }
    );
    assert_eq!(shown_page(&json!({"pageId": 7, "entry": null})), ShownPage::default());
    assert_eq!(shown_page(&json!("not a record")), ShownPage::default());
}

#[test]
fn puts_the_page_on_the_machine_it_was_shown_on_and_names_none_without_an_id() {
    assert_eq!(page_url("http://127.0.0.1:6411/", &page("pg_1", "a.html")).as_deref(), Some("http://127.0.0.1:6411/pages/pg_1/a.html"));
    assert_eq!(page_url("http://127.0.0.1:6411", &page("", "a.html")), None);
    assert_eq!(page_url("http://127.0.0.1:6411", &page("pg_1", "")), None);
}
