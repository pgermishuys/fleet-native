//! The designer's canvas state (component 18, piece O; `WorkflowEditor.vue`'s `selected` and `WorkflowCanvas.vue`'s `addAt`):
//! which step is open in the inspector, which + has its menu open, and where the view measured the nodes, so the loops'
//! arcs can be drawn from them. Plain state; `view::workflows::canvas` draws it.
use super::draft::{insert_step, new_agent_step, new_you_step};
use super::editor::Editor;
use crate::State;

/// What + adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Add {
    Agent,
    You,
}

#[derive(Debug, Default)]
pub struct Canvas {
    /// The step open in the inspector; none is the workflow's own settings.
    pub selected: Option<usize>,
    /// The connector whose menu is open.
    pub add_at: Option<usize>,
    /// Where the flow and each step's node were laid out, in window points (`[x, y, w, h]`), as `on_layout` reports them.
    flow: [f32; 4],
    nodes: Vec<[f32; 4]>,
    /// The canvas's own size: the flow's width and margin, and the dotted ground, follow it.
    pub size: [f32; 2],
}

fn moved(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).any(|(x, y)| (x - y).abs() > 0.25)
}

impl Canvas {
    pub fn set_flow(&mut self, rect: [f32; 4]) {
        if moved(self.flow, rect) {
            self.flow = rect;
        }
    }

    pub fn set_node(&mut self, index: usize, rect: [f32; 4]) {
        if self.nodes.len() <= index {
            self.nodes.resize(index + 1, [0.0; 4]);
        }
        if moved(self.nodes[index], rect) {
            self.nodes[index] = rect;
        }
    }

    pub fn set_size(&mut self, size: [f32; 2]) {
        if (self.size[0] - size[0]).abs() > 0.25 || (self.size[1] - size[1]).abs() > 0.25 {
            self.size = size;
        }
    }

    /// Where step `index`'s node is, from the flow's top left (`[x, y, w, h]`), once both have been laid out.
    pub fn place(&self, index: usize) -> Option<[f32; 4]> {
        let n = self.nodes.get(index).filter(|n| n[2] > 0.0)?;
        (self.flow[2] > 0.0).then(|| [n[0] - self.flow[0], n[1] - self.flow[1], n[2], n[3]])
    }

    pub fn flow_size(&self) -> [f32; 2] {
        [self.flow[2], self.flow[3]]
    }

    /// A step that's gone (removed, or the file reloaded) leaves the workflow's settings open (14.7).
    pub fn keep(&mut self, steps: usize) {
        if self.selected.is_some_and(|i| i >= steps) {
            self.selected = None;
        }
        if self.add_at.is_some_and(|i| i > steps) {
            self.add_at = None;
        }
        self.nodes.truncate(steps);
    }

    /// Another file opened: nothing is picked. What was measured stays, since Rune reports a node's place only when it moves.
    pub fn reopen(&mut self) {
        self.selected = None;
        self.add_at = None;
    }

    /// `toggleAdd`.
    pub fn toggle_add(&mut self, slot: usize) {
        self.add_at = if self.add_at == Some(slot) { None } else { Some(slot) };
    }
}

impl Editor {
    /// `insert`: the new step goes before `index` and opens in the inspector.
    pub fn insert_canvas_step(&mut self, index: usize, kind: Add) {
        let Some(draft) = self.draft.clone() else { return };
        let step = match kind {
            Add::Agent => new_agent_step(&draft),
            Add::You => new_you_step(&draft, draft.steps.get(index).map(|s| s.id.as_str())),
        };
        self.edit_draft(insert_step(&draft, index, step));
        self.canvas.selected = Some(index);
    }
}

impl State {
    /// A click on a + menu's item.
    pub fn workflow_canvas_add(&mut self, kind: Add) {
        let ed = &mut self.workflows_nav.editor;
        if let Some(at) = ed.canvas.add_at.take() {
            ed.insert_canvas_step(at, kind);
        }
    }
}
