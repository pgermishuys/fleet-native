//! Finding a running Fleet in every place one writes itself down (`fleet::discover`): the folders on each
//! system, the order they are tried in, the instance file that is missing, and the wording of what the
//! window says. Temporary folders and a fake environment; nothing reads the real home folder.
use crate::fleet::discover::{self, Os, Place, desktop_user_data_dir, explicit_server, find_in, places, saved_port, server_data_dir};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-discover-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A server that answers `/readyz` with 200 for as long as the test runs.
fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        while let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok");
        }
    });
    url
}

fn dead_url() -> String {
    format!("http://127.0.0.1:{}", TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port())
}

fn instance_json(url: &str) -> String {
    serde_json::json!({"pid": std::process::id(), "url": url, "version": "1.0.0"}).to_string()
}

#[test]
fn the_server_keeps_its_data_in_the_local_app_data_folder_of_each_system() {
    let linux = server_data_dir(Os::Linux, &env(&[("HOME", "/home/me")])).unwrap();
    assert_eq!(linux, Path::new("/home/me/.local/share/WeaveFleet"));
    // XDG_DATA_HOME moves it, as .NET does; a relative one is ignored.
    let xdg = server_data_dir(Os::Linux, &env(&[("HOME", "/home/me"), ("XDG_DATA_HOME", "/data")])).unwrap();
    assert_eq!(xdg, Path::new("/data/WeaveFleet"));
    let relative = server_data_dir(Os::Linux, &env(&[("HOME", "/home/me"), ("XDG_DATA_HOME", "data")])).unwrap();
    assert_eq!(relative, Path::new("/home/me/.local/share/WeaveFleet"));
    let mac = server_data_dir(Os::Mac, &env(&[("HOME", "/Users/me")])).unwrap();
    assert_eq!(mac, Path::new("/Users/me/Library/Application Support/WeaveFleet"));
    let win = server_data_dir(Os::Windows, &env(&[("USERPROFILE", "/c/Users/me"), ("LOCALAPPDATA", "/d/Local")])).unwrap();
    assert_eq!(win, Path::new("/d/Local").join("WeaveFleet"));
    // Without LOCALAPPDATA, the usual place under the profile.
    let win = server_data_dir(Os::Windows, &env(&[("USERPROFILE", "/c/Users/me")])).unwrap();
    assert_eq!(win, Path::new("/c/Users/me").join("AppData").join("Local").join("WeaveFleet"));
    assert_eq!(server_data_dir(Os::Mac, &env(&[])), None);
}

#[test]
fn the_desktop_app_keeps_its_settings_under_its_product_name_on_each_system() {
    assert_eq!(
        desktop_user_data_dir(Os::Mac, &env(&[("HOME", "/Users/me")])).unwrap(),
        Path::new("/Users/me/Library/Application Support/Fleet")
    );
    assert_eq!(desktop_user_data_dir(Os::Linux, &env(&[("HOME", "/home/me")])).unwrap(), Path::new("/home/me/.config/Fleet"));
    assert_eq!(
        desktop_user_data_dir(Os::Linux, &env(&[("HOME", "/home/me"), ("XDG_CONFIG_HOME", "/cfg")])).unwrap(),
        Path::new("/cfg/Fleet")
    );
    assert_eq!(
        desktop_user_data_dir(Os::Windows, &env(&[("USERPROFILE", "/c/Users/me"), ("APPDATA", "/d/Roaming")])).unwrap(),
        Path::new("/d/Roaming").join("Fleet")
    );
    assert_eq!(
        desktop_user_data_dir(Os::Windows, &env(&[("USERPROFILE", "/c/Users/me")])).unwrap(),
        Path::new("/c/Users/me").join("AppData").join("Roaming").join("Fleet")
    );
}

#[test]
fn the_places_are_tried_in_order_on_each_system() {
    let mac = places(Os::Mac, &env(&[("HOME", "/Users/me")]));
    assert_eq!(
        mac,
        vec![
            Place::InstanceFile("/Users/me/.weave/fleet.instance.json".into()),
            Place::InstanceFile("/Users/me/Library/Application Support/WeaveFleet/fleet.instance.json".into()),
            Place::SavedPort("/Users/me/Library/Application Support/Fleet/desktop-settings.json".into()),
        ]
    );
    let linux = places(Os::Linux, &env(&[("HOME", "/home/me")]));
    assert_eq!(linux[0], Place::InstanceFile("/home/me/.weave/fleet.instance.json".into()));
    assert_eq!(linux[1], Place::InstanceFile("/home/me/.local/share/WeaveFleet/fleet.instance.json".into()));
    assert_eq!(linux[2], Place::SavedPort("/home/me/.config/Fleet/desktop-settings.json".into()));
    let win = places(Os::Windows, &env(&[("USERPROFILE", "/c/Users/me"), ("LOCALAPPDATA", "/c/L"), ("APPDATA", "/c/R")]));
    assert_eq!(win.len(), 3);
    assert_eq!(win[0], Place::InstanceFile(Path::new("/c/Users/me").join(".weave").join("fleet.instance.json")));
    assert_eq!(win[1], Place::InstanceFile(Path::new("/c/L").join("WeaveFleet").join("fleet.instance.json")));
    assert_eq!(win[2], Place::SavedPort(Path::new("/c/R").join("Fleet").join("desktop-settings.json")));
}

#[test]
fn the_overrides_come_first() {
    let got = places(
        Os::Linux,
        &env(&[
            ("HOME", "/home/me"),
            ("FLEET_DESKTOP_DATA_DIR", "/desk"),
            ("Fleet__DatabasePath", "/db/my.fleet.db"),
            ("WEAVE_FLEET_DATA_DIR", "/cmd"),
        ]),
    );
    // The server's own override, with the extension swapped as `FleetInstanceLock` does.
    assert_eq!(got[0], Place::InstanceFile("/db/my.fleet.instance.json".into()));
    assert_eq!(got[1], Place::InstanceFile("/cmd/fleet.instance.json".into()));
    assert_eq!(got[2], Place::InstanceFile("/desk/fleet.instance.json".into()));
    assert_eq!(got[3], Place::InstanceFile("/home/me/.local/share/WeaveFleet/fleet.instance.json".into()));
    // The same place twice is tried once.
    let same = places(Os::Linux, &env(&[("HOME", "/home/me"), ("FLEET_DESKTOP_DATA_DIR", "/home/me/.local/share/WeaveFleet")]));
    assert_eq!(same.len(), 2);
    // The launcher's folder is a place of its own.
    let cmd = places(Os::Linux, &env(&[("HOME", "/home/me"), ("WEAVE_FLEET_DATA_DIR", "/cmd")]));
    assert_eq!(cmd.len(), 4);
}

#[test]
fn with_no_home_nothing_is_tried_but_the_port() {
    assert!(places(Os::Linux, &env(&[])).is_empty());
    let all = discover::all_places(Os::Linux, &env(&[]));
    assert_eq!(all, vec![Place::Port(6262)]);
}

#[test]
fn the_saved_port_is_a_number_in_range() {
    assert_eq!(saved_port(r#"{"port": 6263, "window": {"x": 1}}"#), Some(6263));
    for bad in ["", "not json", "[]", r#"{}"#, r#"{"port": "6263"}"#, r#"{"port": 0}"#, r#"{"port": 70000}"#, r#"{"port": -1}"#] {
        assert_eq!(saved_port(bad), None, "{bad}");
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn each_place_finds_the_fleet_it_names() {
    // Run for the system we are on, with a fake home; the layout for the others is covered above.
    let os = Os::current();
    let home = temp("each");
    let home_s = home.to_string_lossy().to_string();
    let vars: Vec<(&str, &str)> = vec![
        ("HOME", &home_s),
        ("USERPROFILE", &home_s),
        ("LOCALAPPDATA", ""),
        ("APPDATA", ""),
        ("XDG_DATA_HOME", ""),
        ("XDG_CONFIG_HOME", ""),
    ];
    let all = places(os, &env(&vars));
    let url = server();
    for (i, place) in all.iter().enumerate() {
        // Only this place says where the Fleet is.
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(find_in(&all), None, "nothing written down");
        match place {
            Place::InstanceFile(file) => write(file, &instance_json(&url)),
            Place::SavedPort(file) => write(file, &format!(r#"{{"port": {}}}"#, url.rsplit(':').next().unwrap())),
            Place::Port(_) => unreachable!(),
        }
        let found = find_in(&all).unwrap_or_else(|| panic!("place {i} ({place:?}) found nothing"));
        assert_eq!((found.url.as_str(), &found.place), (url.as_str(), place));
    }
}

#[test]
fn the_first_place_that_answers_wins_and_a_stale_one_is_skipped() {
    let dir = temp("order");
    let (a, b, c) = (dir.join("a/fleet.instance.json"), dir.join("b/fleet.instance.json"), dir.join("c/desktop-settings.json"));
    let (url_b, url_c) = (server(), server());
    write(&a, &instance_json(&dead_url())); // the file is there but its Fleet is gone
    write(&b, &instance_json(&url_b));
    write(&c, &format!(r#"{{"port": {}}}"#, url_c.rsplit(':').next().unwrap()));
    let all = vec![Place::InstanceFile(a.clone()), Place::InstanceFile(b.clone()), Place::SavedPort(c.clone())];
    assert_eq!(find_in(&all).unwrap().url, url_b);
    // Drop b and the saved port is next.
    assert_eq!(find_in(&[all[0].clone(), all[2].clone()]).unwrap().url, url_c);
    // A saved port nobody answers on finds nothing.
    write(&c, &format!(r#"{{"port": {}}}"#, dead_url().rsplit(':').next().unwrap()));
    assert_eq!(find_in(&[all[0].clone(), all[2].clone()]), None);
    // The preferred port is tried last.
    let port: u16 = url_b.rsplit(':').next().unwrap().parse().unwrap();
    assert_eq!(find_in(&[all[0].clone(), Place::Port(port)]).unwrap().url, url_b);
}

#[test]
fn the_instance_file_may_be_missing_while_a_fleet_runs() {
    // A desktop app whose Fleet took another port writes `desktop-settings.json` but there is nothing in
    // `~/.weave`: the saved port still finds it.
    let home = temp("missing");
    let url = server();
    let settings = home.join("cfg/Fleet/desktop-settings.json");
    write(&settings, &format!(r#"{{"port": {}, "window": {{"width": 900}}}}"#, url.rsplit(':').next().unwrap()));
    let all = vec![Place::InstanceFile(home.join(".weave/fleet.instance.json")), Place::SavedPort(settings)];
    assert_eq!(find_in(&all).unwrap().url, url);
}

#[cfg(unix)]
#[test]
fn the_notice_says_where_it_looked_and_how_to_name_a_fleet() {
    use crate::fleet::discover::not_found;
    let looked = places(Os::Mac, &env(&[("HOME", "/Users/me")]));
    let notice = not_found(&[looked.clone(), vec![Place::Port(6262)]].concat());
    assert_eq!(
        notice,
        "No Fleet found (looked in /Users/me/.weave, /Users/me/Library/Application Support/WeaveFleet, /Users/me/Library/Application Support/Fleet, port 6262). \
         Start Fleet, or use --server <url> and --token <token> (or FLEET_NATIVE_SERVER and FLEET_NATIVE_TOKEN)."
    );
}

#[test]
fn server_names_the_address_by_argument_then_by_environment() {
    let vars = env(&[("FLEET_NATIVE_SERVER", " http://127.0.0.1:6300 ")]);
    assert_eq!(explicit_server(Some("http://127.0.0.1:1".into()), &vars).as_deref(), Some("http://127.0.0.1:1"));
    assert_eq!(explicit_server(None, &vars).as_deref(), Some("http://127.0.0.1:6300"));
    assert_eq!(explicit_server(None, &env(&[("FLEET_NATIVE_SERVER", "")])), None);
    assert_eq!(explicit_server(None, &env(&[])), None);
}

// ─── A normal start holds nothing invented ──────────────────────────────────

#[test]
fn a_normal_start_with_no_fleet_holds_no_mock_data() {
    use crate::tests::{FRAME, app};
    let mut rt = app();
    // The mock window a scene or `--demo` shows, to be sure the test starts from something invented.
    assert!(rt.state.has_rows() && rt.state.session().tokens > 0);
    // What `State::default` does on a normal start, then discovery's answer.
    rt.state.go_live(true);
    rt.frame(FRAME);
    assert!(rt.state.list_is_loading(), "the list says loading while it looks");
    rt.state.on_no_fleet("No Fleet found.".into());
    rt.frame(FRAME);
    let s = &rt.state;
    assert!(s.projects.is_empty() && !s.has_rows() && !s.has_session());
    assert!(s.session().messages.is_empty());
    assert_eq!(s.session().title, "No session open");
    assert!(s.lists().agents.is_empty() && s.lists().models.is_empty());
    assert!(!s.list_is_loading() && s.no_fleet && s.live && !s.is_mock());
    assert_eq!(s.connection_note().as_deref(), Some("No Fleet found."));
}
