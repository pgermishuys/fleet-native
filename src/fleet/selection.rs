//! Rows picked in the session list for a bulk action (`stores/session-selection.ts`, rows 1.48-1.53). Ctrl/Cmd-click toggles one
//! row and Shift-click takes the range from the last picked row, as file lists do. The rows picked are all on one machine
//! (`None` is the live machine's tree, another machine's key is its group): picking a row on another machine starts again.
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Selection {
    /// In the order they were picked, which is the order a bulk archive sends them in (a JS `Set` keeps insertion order).
    ids: Vec<String>,
    anchor: Option<String>,
    machine: Option<String>,
    /// The ids each machine's tree shows, top to bottom; a Shift-click range is taken from it.
    orders: HashMap<Option<String>, Vec<String>>,
}

impl Selection {
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn count(&self) -> usize {
        self.ids.len()
    }

    pub fn is_selecting(&self) -> bool {
        !self.ids.is_empty()
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }

    /// Whether rows are being picked in `machine`'s tree.
    pub fn is_selecting_on(&self, machine: Option<&str>) -> bool {
        self.is_selecting() && self.machine.as_deref() == machine
    }

    /// The machine the picked rows are on (`machineKey`): `None` for the live machine's tree.
    pub fn machine(&self) -> Option<&str> {
        self.machine.as_deref()
    }

    /// Rows picked on another machine are let go when one on `machine` is picked (`selectionOn`).
    fn on(&mut self, machine: Option<&str>) {
        if self.machine.as_deref() != machine {
            self.machine = machine.map(str::to_string);
            self.anchor = None;
            self.ids.clear();
        }
    }

    pub fn toggle(&mut self, id: &str, machine: Option<&str>) {
        self.on(machine);
        match self.ids.iter().position(|i| i == id) {
            Some(at) => {
                self.ids.remove(at);
            }
            None => self.ids.push(id.to_string()),
        }
        self.anchor = Some(id.to_string());
    }

    /// Adds every row between the anchor (or `fallback`, usually the open session) and this one.
    pub fn extend_to(&mut self, id: &str, fallback: Option<&str>, machine: Option<&str>) {
        if self.machine.as_deref() != machine {
            self.toggle(id, machine);
            return;
        }
        let order = self.orders.get(&machine.map(str::to_string)).map(Vec::as_slice).unwrap_or(&[]);
        let from_id = self.anchor.as_deref().or(fallback).unwrap_or(id);
        let (from, to) = (order.iter().position(|o| o == from_id), order.iter().position(|o| o == id));
        // An anchor or a target the list doesn't show falls back to a toggle.
        let (Some(from), Some(to)) = (from, to) else {
            self.toggle(id, machine);
            return;
        };
        let range: Vec<String> = order[from.min(to)..=from.max(to)].to_vec();
        for r in range {
            if !self.is_selected(&r) {
                self.ids.push(r);
            }
        }
        self.anchor = Some(id.to_string());
    }

    /// What `machine`'s tree shows now. A picked row that left it (archived, filtered out, folded) leaves the selection too.
    /// True when the selection lost a row.
    pub fn set_visible_order(&mut self, order: Vec<String>, machine: Option<&str>) -> bool {
        let mut lost = false;
        if self.is_selecting() && self.machine.as_deref() == machine {
            let before = self.ids.len();
            self.ids.retain(|id| order.contains(id));
            lost = self.ids.len() != before;
        }
        self.orders.insert(machine.map(str::to_string), order);
        lost
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
        self.machine = None;
    }
}
