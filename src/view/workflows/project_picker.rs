//! `ProjectPicker.vue`: the Project chip of the Run box and the menu above it: Scratch, then each of the user's projects, a
//! check on the one picked. The chip is `Project: <name>`; null is Scratch.
use crate::fleet::workflows::run_box::{PROJECT_CHIP, PROJECT_POPOVER};
use crate::theme::Theme;
use crate::ui::chip::{Chip, Option_, option, pop, pop_label};
use crate::{State, Tx};
use rune::{Align, El, Side};

const WIDTH: f32 = 220.0;

pub fn chip(s: &State, t: &Theme) -> El {
    let run_box = &s.workflows_nav.run_box;
    let name =
        s.run_box_projects().into_iter().find(|p| Some(&p.id) == run_box.project_id.as_ref()).map_or("Scratch".to_string(), |p| p.name);
    Chip::new(PROJECT_CHIP, name)
        .head("Project: ")
        .open(s.run_box_project_menu_open())
        .disabled(run_box.starting)
        .help(Some("Where the run's sessions go".into()))
        .view(t, |s: &mut State, _: &mut Tx| s.ui.open = Some(crate::ui::Open::Popover(PROJECT_POPOVER)))
}

/// `Project`, then Scratch and the user's projects.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let picked = s.workflows_nav.run_box.project_id.clone();
    let mut body = vec![pop_label(t, "Project")];
    let scratch = Option_ { checked: picked.is_none(), no_mark: true, ..Option_::new("workflow-project-scratch", "Scratch") };
    body.push(option(t, scratch, None, |s, _| s.run_box_choose_project(None)));
    for project in s.run_box_projects() {
        let id = project.id.clone();
        let spec = Option_ {
            checked: picked.as_deref() == Some(project.id.as_str()),
            no_mark: true,
            ..Option_::new(format!("workflow-project-{}", project.id), project.name)
        };
        body.push(option(t, spec, None, move |s, _| s.run_box_choose_project(Some(id.clone()))));
    }
    let panel = pop(t, "workflow-project-pop", WIDTH, body)
        .focus_return(PROJECT_CHIP.to_string())
        .modal(true, |s: &mut State| s.close_run_box_menu());
    vec![
        crate::ui::overlay::scrim("workflow-project-scrim", |s: &mut State| s.close_run_box_menu()),
        crate::ui::overlay::anchored(panel, PROJECT_CHIP, Side::Top, Align::Start, 6.0),
    ]
}
