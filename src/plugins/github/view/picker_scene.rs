//! Scenes of the `#` popup (`new-session-hash*`): the New session page with `Fix #` typed in a repository whose remote is GitHub's,
//! drawn from invented items with no server.
use crate::State;
use crate::fleet::new_session::request::Folder;
use crate::plugins::github::items::ItemSummary;
use crate::plugins::github::picker::Group;
use crate::plugins::github::{mock, state};

/// `which`: `` the list, `loading` the first question still out, `no-repo` a folder with no GitHub remote.
pub fn set(s: &mut State, which: &str, dark: bool) {
    s.dark = Some(dark);
    s.open_new_session_page(None);
    s.set_folder(Folder::Repository(crate::mock::feeds::ROCKET.into()), true);
    let mut detail = crate::mock::feeds::detail();
    detail.remotes = match which {
        "no-repo" => vec![crate::fleet::new_session::feeds::Remote {
            name: "origin".into(),
            url: "git@example.com:ada/rocket.git".into(),
            github: None,
        }],
        _ => vec![crate::fleet::new_session::feeds::Remote {
            name: "origin".into(),
            url: "https://github.com/acme/rocket.git".into(),
            github: Some(("acme".into(), "rocket".into())),
        }],
    };
    s.feeds.detail.path = Some(crate::mock::feeds::ROCKET.into());
    s.feeds.detail.value = Some(detail);
    if let Some(d) = s.new_draft.draft_mut() {
        d.message = "Fix #".into();
    }
    s.sync_new_message();
    let end = s.new_box.edit.text.len();
    s.new_box.edit.set_caret(end);
    let items: Vec<ItemSummary> = mock::picker_items().as_array().into_iter().flatten().map(ItemSummary::read).collect();
    let p = &mut state(&mut s.places).picker;
    match which {
        "loading" => p.show(Vec::new(), true),
        "no-repo" => {}
        _ => {
            let (mine, rest) = items.split_at(2);
            p.show(
                vec![Group { label: "Assigned to you", items: mine.to_vec() }, Group { label: "Recently updated", items: rest.to_vec() }],
                false,
            );
        }
    }
}
