//! New job (`NewJobDialog.vue`): the kit's dialog with a title, the ask, the repository to run in, what it needs and where it
//! runs, an error note, Cancel and Post job. The state is `fleet::jobs::new_job`.
use crate::State;
use crate::fleet::jobs::new_job::{ASK_FIELD, LABELS_FIELD, NewJobForm, Pick, TITLE_FIELD, repository_label};
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::controls::{grow, input, textarea_field};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::{error_note, rem, select};
use rune::{El, Key, KeyEvent, Tx, col, row, text};
use std::rc::Rc;

pub const REPOSITORY: &str = "new-job-repository";
pub const MACHINE: &str = "new-job-machine";
/// `DialogContent`'s padding takes 2 × 1.5rem off its width.
const INNER: f32 = 32.0 * 14.0 - 2.0 * 21.0;
const NO_REPOSITORIES: &str = "No machine that takes jobs has a repository yet. Give a machine seats in Settings → Machines.";

fn label(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(t.text)
}

fn field(t: &Theme, words: &str, control: El) -> El {
    col().gap(rem(0.375)).w_full().child(label(t, words)).child(control)
}

/// Enter in a one-line field posts (`<form @submit>`).
fn enter_posts(e: El) -> El {
    e.on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
        if ev.key == Key::Enter {
            s.new_job_submit();
            return true;
        }
        false
    })
}

fn form_mut(s: &mut State) -> &mut NewJobForm {
    s.jobs.new.as_mut().expect("the dialog is open")
}

fn body(s: &State, t: &Theme, f: &NewJobForm) -> Vec<El> {
    let repositories = s.jobs.repositories.list.as_deref().unwrap_or_default();
    let repo_label = repositories.iter().find(|r| r.repository == f.repository).map(repository_label).unwrap_or_default();
    let repo = select::trigger(t, &repo_label, false, false, INNER)
        .key(REPOSITORY)
        .on_press(|s: &mut State| form_mut(s).picking = Some(Pick::Repository));
    let mut repo_field = col().gap(rem(0.375)).w_full().child(label(t, "Repository")).child(repo);
    if repositories.is_empty() {
        repo_field = repo_field
            .child(text(NO_REPOSITORIES).key("new-job-no-repositories").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full());
    }
    let half = (INNER - rem(0.75)) / 2.0;
    let machine_label = s.jobs.machines.iter().find(|m| m.id == f.pinned).map_or("Any machine that fits", |m| m.name.as_str()).to_string();
    let machine = select::trigger(t, &machine_label, false, false, half)
        .key(MACHINE)
        .on_press(|s: &mut State| form_mut(s).picking = Some(Pick::Machine));
    let labels = enter_posts(input(t, &f.labels, "mac, gpu").key(LABELS_FIELD).on_edit(|s: &mut State| &mut form_mut(s).labels));
    let title = enter_posts(input(t, &f.title, "Add rate limits").key(TITLE_FIELD).on_edit(|s: &mut State| &mut form_mut(s).title));
    let ask = grow(textarea_field(t, &f.ask, "What the agent should do").key(ASK_FIELD).on_edit(|s: &mut State| &mut form_mut(s).ask), 8);
    let mut form = col()
        .key("new-job-form")
        .w_full()
        .gap(rem(1.0))
        .child(field(t, "Title", title))
        .child(field(t, "Ask", ask))
        .child(repo_field)
        .child(
            row().w_full().gap(rem(0.75)).items_start().child(field(t, "Needs", labels).w(half)).child(field(t, "Run on", machine).w(half)),
        );
    if let Some(why) = &f.error {
        form = form.child(error_note::message(t, "new-job-error", why));
    }
    vec![form]
}

/// The lists of the two selects, anchored under their triggers.
fn menu(s: &State, t: &Theme, f: &NewJobForm, which: Pick) -> Vec<El> {
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| {
        if let Some(f) = s.jobs.new.as_mut() {
            f.picking = None;
        }
    });
    match which {
        Pick::Repository => {
            let labels: Vec<(String, String)> =
                s.jobs.repositories.list.iter().flatten().map(|r| (r.repository.clone(), repository_label(r))).collect();
            let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
            select::content_in(
                t,
                "new-job-repository",
                REPOSITORY,
                INNER,
                &options,
                Some(&f.repository),
                Rc::new(|s: &mut State, v: &str| form_mut(s).repository = v.to_string()),
                close,
                0.0,
            )
        }
        Pick::Machine => {
            let mut labels = vec![(String::new(), "Any machine that fits".to_string())];
            labels.extend(s.jobs.machines.iter().map(|m| (m.id.clone(), m.name.clone())));
            let options: Vec<(&str, &str)> = labels.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
            select::content_in(
                t,
                "new-job-machine",
                MACHINE,
                (INNER - rem(0.75)) / 2.0,
                &options,
                Some(&f.pinned),
                Rc::new(|s: &mut State, v: &str| form_mut(s).pinned = v.to_string()),
                close,
                0.0,
            )
        }
    }
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let f = s.jobs.new.as_ref()?;
    let posting = s.jobs.post.busy;
    let cancel =
        button(t, Variant::Outline, Size::Default, None, "Cancel").key("new-job-cancel").on_press(|s: &mut State| s.jobs.new = None);
    let post = if posting {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg)), "Post job")
    } else {
        button(t, Variant::Default, Size::Default, None, "Post job")
    }
    .key("new-job-post");
    let post = if f.can_post(posting) { post.on_press(|s: &mut State| s.new_job_submit()) } else { disabled(post) };
    let mut layer = dialog(
        t,
        Dialog {
            key: "new-job",
            title: "New job".into(),
            description: Some("Fleet starts it on a machine with a free seat. The ask goes to the agent as you write it.".into()),
            body: body(s, t, f),
            footer: vec![cancel, post],
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_new_job()),
            ..Default::default()
        },
    );
    if let Some(which) = f.picking {
        layer.extend(menu(s, t, f, which));
    }
    Some(layer)
}
