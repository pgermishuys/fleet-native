//! "Session not found": what a session's column shows when its machine has no session under the id (a link from another Fleet,
//! or a session deleted since). Ports the `data-testid="session-not-found"` block of `routes/sessions.$id.tsx`.
use crate::State;
use crate::icons::ARROW_LEFT;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use rune::{Align, El, col, el, text};

pub fn column(t: &Theme) -> El {
    col()
        .key("session-not-found")
        .label("Session not found")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .center()
        .gap(8.0)
        .px(24.0)
        .child(text("Session not found").font(sans(rem(1.0), 600.0, 1.4)).color(t.text))
        .child(
            text("It may have been deleted, or the link is from another Fleet.")
                .font(sans(rem(0.875), 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .align(Align::Center)
                // `max-w-sm` is 336 here, but Rune sets this line about 4 % wider than Chrome does and 336 wrapped it where Fleet's
                // stays on one line.
                .max_w(352.0),
        )
        .child(
            el().pt(8.0).child(
                button(t, Variant::Outline, Size::Sm, Some(&ARROW_LEFT), "Back to sessions")
                    .key("session-not-found-back")
                    .on_press(|s: &mut State| s.leave_missing_session()),
            ),
        )
}

impl State {
    /// "Back to sessions" (`leaveMissingSession`): home, `/`.
    pub fn leave_missing_session(&mut self) {
        self.go(crate::fleet::store::Route::Home);
    }
}
