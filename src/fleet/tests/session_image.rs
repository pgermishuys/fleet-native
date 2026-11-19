//! An image of a session's folder over a fake server (component 11, piece I): the bytes are asked of `files/image` on a thread, a 404
//! and another refusal say different things, and the image is read again when its tab comes back to the front.
use super::super::canvases::OpenFile;
use super::super::link::{Job, Link, Post};
use super::super::session_image::{GONE, Shown, UNREADABLE};
use super::reconnect::app;
use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const LOGO: &str = include_str!("../../mock/file_logo.svg");

#[derive(Default)]
struct Seen {
    asked: Vec<String>,
    /// The status the next answer for the image carries.
    status: u16,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let path = heard.head.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
    let (status, kind, body): (u16, &str, Vec<u8>) = if path.starts_with("/hubs/session-events/negotiate") {
        (200, "application/json", b"{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_vec())
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        (200, "application/json", fixture("api-sessions.json").into_bytes())
    } else if let Some((_, asked)) = path.split_once("/files/image?path=") {
        let mut seen = seen.lock().unwrap();
        seen.asked.push(asked.to_string());
        match seen.status {
            200 => (200, "image/svg+xml", LOGO.as_bytes().to_vec()),
            code => (code, "application/json", b"{}".to_vec()),
        }
    } else {
        (200, "application/json", b"{}".to_vec())
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 {status} X\r\ncontent-type: {kind}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len()).as_bytes(),
    );
    let _ = stream.write_all(&body);
}

#[test]
fn an_image_is_read_from_the_sessions_machine_and_again_when_its_tab_comes_back() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen { status: 200, ..Seen::default() }));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = shared.clone();
            std::thread::spawn(move || serve(stream, seen));
        }
    });
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let post = Post::new(move |job| {
        let _ = tx.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, tx| {
        s.link = Some(link);
        s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
    });
    let id = rt.state.open_id().unwrap();
    let machine = rt.state.machine_key_of(&id).to_string();
    let state_of = {
        let (id, machine) = (id.clone(), machine.clone());
        move |s: &crate::State| match s.session_images.shown(&machine, &id, "assets/logo.svg") {
            Shown::Opening => "opening".to_string(),
            Shown::Note(why) => why.to_string(),
            Shown::Picture { meta, .. } => meta,
        }
    };
    let session = id.clone();
    rt.update(move |s, tx| s.open_file(&session, "assets/logo.svg", OpenFile { keep: true, view: None }, tx));
    // Make the window's image cache exist the way `setup` does, so bytes are decoded.
    let described = seen.clone();
    let look = state_of.clone();
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the image to be read and decoded",
        move |s| look(s).ends_with(&format!("{} B", LOGO.len())),
        move |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().asked),
    );
    assert_eq!(seen.lock().unwrap().asked, ["assets%2Flogo.svg"], "one request for the open");
    let session = id.clone();
    assert!(rt.state.file_buffers.info(&machine, &session, "assets/logo.svg").is_none(), "no text read of a picture");

    // The tab comes to the front again: the picture is read again, and a 404 now says the file is gone.
    seen.lock().unwrap().status = 404;
    rt.update(|s, tx| {
        let tab = s.active_canvas();
        s.activate_tab(&tab, tx);
    });
    let (described, look) = (seen.clone(), state_of.clone());
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the second read to say the file is gone",
        move |s| look(s) == GONE,
        move |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().asked),
    );
    assert_eq!(seen.lock().unwrap().asked.len(), 2);

    seen.lock().unwrap().status = 500;
    rt.update(|s, tx| {
        let tab = s.active_canvas();
        s.activate_tab(&tab, tx);
    });
    let look = state_of.clone();
    crate::test_wait::drive(&mut rt, &jobs, "the third read to fail", move |s| look(s) == UNREADABLE, crate::test_wait::app_state);
    assert_eq!(seen.lock().unwrap().asked.len(), 3);
}
