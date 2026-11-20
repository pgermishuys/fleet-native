//! The plugin's addresses (`routes/github*.tsx`): what each route names, and the way back to the address.
use super::where_::{Kind, Where};

#[test]
fn each_route_of_the_plugin_names_a_place_and_back() {
    let repo = Where::Repo { owner: "acme".into(), repo: "rocket".into() };
    let pull = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind: Kind::Pull, number: 218 };
    let issue = Where::Item { owner: "acme".into(), repo: "rocket".into(), kind: Kind::Issue, number: 7 };
    for (path, place) in [
        ("/github", Where::Home),
        ("/github/acme/rocket", repo),
        ("/github/acme/rocket/pulls/218", pull),
        ("/github/acme/rocket/issues/7", issue),
    ] {
        assert_eq!(Where::parse(path), Some(place.clone()), "{path}");
        assert_eq!(place.path(), path);
    }
}

#[test]
fn an_address_that_is_not_the_plugins_names_nothing() {
    for path in [
        "/",
        "/githubby",
        "/github/acme",
        "/github/acme/rocket/pulls",
        "/github/acme/rocket/pulls/x",
        "/github/acme/rocket/wiki/1",
        "/github//rocket",
        "/github/a/b/pulls/1/more",
    ] {
        assert_eq!(Where::parse(path), None, "{path}");
    }
    assert_eq!(Where::parse("/github/"), Some(Where::Home), "a trailing slash is the same address");
}
