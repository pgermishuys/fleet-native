//! Go to file's store (weave-fleet `stores/go-to-file.ts`): which session's picker is open, and which file should take the
//! keyboard once it is drawn. A file opened from a message's link takes it the same way, at the line the message named.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoToFile {
    /// The session whose picker is open.
    pub session: Option<String>,
    /// The file to give the keyboard to when its view opens, with the line to show.
    pub focus_on_open: Option<FocusOnOpen>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusOnOpen {
    pub session: String,
    pub path: String,
    pub line: Option<u32>,
}

impl GoToFile {
    pub fn show(&mut self, session: &str) {
        self.session = Some(session.to_string());
    }

    pub fn hide(&mut self) {
        self.session = None;
    }

    pub fn ask_focus(&mut self, session: &str, path: &str, line: Option<u32>) {
        self.focus_on_open = Some(FocusOnOpen { session: session.to_string(), path: path.to_string(), line });
    }

    /// The file's view calls this when it is drawn: the request once, for that session and path only, else `None`. The inner
    /// option is the line (`takeFocus`).
    pub fn take_focus(&mut self, session: &str, path: &str) -> Option<Option<u32>> {
        let pending = self.focus_on_open.as_ref()?;
        if pending.session != session || pending.path != path {
            return None;
        }
        self.focus_on_open.take().map(|p| p.line)
    }
}
