//! A machine's address list on its card (`MachineAddressList.vue`, `stores/machines.ts` `reorderAddresses`): the order Fleet tries
//! the addresses in. A row moves by Alt+Up / Alt+Down on its handle (plain Up / Down too) or by dragging it; the new order shows at
//! once, `PUT /api/machines/{id}` `{addresses}` keeps it, and a refusal puts the old order back and says why under the card.
//! With no server (scenes, tests) the order is changed on the invented list.
use super::machine::move_address;
use super::machine_actions::{machine_path, send};
use crate::{State, Tx};
use rune::Picked;
use serde_json::json;
use std::collections::HashMap;

/// A row of the list being dragged, and the row under the pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dragging {
    pub key: String,
    pub from: usize,
    pub over: Option<usize>,
}

/// What every card's list keeps while the page is open.
#[derive(Default)]
pub struct Lists {
    /// A machine's urls in the order just asked for, shown until the save is answered (`order` in the component).
    pub order: HashMap<String, Vec<String>>,
    /// The machine whose order is being saved: its handles wait (`saving`).
    pub saving: Option<String>,
    /// What a screen reader is told after a move (`announcement`); not drawn.
    pub announcement: HashMap<String, String>,
    pub drag: Option<Dragging>,
}

pub fn handle_key(card: &str, at: usize) -> String {
    format!("machine-address-handle-{card}-{at}")
}

/// A row is keyed by its place, so a drag can name it from the path under the pointer (urls hold slashes).
pub fn row_key(card: &str, at: usize) -> String {
    format!("machine-address-{card}-{at}")
}

impl State {
    /// A machine's addresses as the card shows them now.
    fn address_urls(&self, key: &str) -> Vec<String> {
        self.machine_cards()
            .into_iter()
            .find(|c| c.key == key)
            .map(|c| c.addresses.into_iter().map(|a| a.url).collect())
            .unwrap_or_default()
    }

    /// Moves the address at `from` to `to` and saves the order (`move`, `commit`). Nothing happens off the list, onto itself or
    /// while an earlier save is out.
    pub fn move_card_address(&mut self, key: &str, from: isize, to: isize, tx: &mut Tx) {
        let before = self.address_urls(key);
        let lists = &mut self.machine_page.addresses;
        if from < 0 || to < 0 || to as usize >= before.len() || from as usize >= before.len() || to == from || lists.saving.is_some() {
            return;
        }
        let next = move_address(&before, from, to);
        let kind = self.machine_cards().into_iter().find(|c| c.key == key).map(|c| c.addresses[from as usize].kind).unwrap_or("Address");
        let lists = &mut self.machine_page.addresses;
        lists.announcement.insert(key.to_string(), format!("{kind} moved to position {} of {}", to + 1, next.len()));
        lists.order.insert(key.to_string(), next.clone());
        self.machine_page.errors.remove(key);
        tx.focus(handle_key(key, to as usize), false);
        let Some(link) = self.link.clone() else {
            self.keep_address_order(key, &next);
            return;
        };
        self.machine_page.addresses.saving = Some(key.to_string());
        let id = key.to_string();
        link.off_thread(
            {
                let (id, next) = (id.clone(), next.clone());
                move |home| {
                    send(
                        "PUT",
                        home,
                        &machine_path(&id),
                        Some(&json!({ "addresses": next })),
                        "Couldn't save the order.",
                        "Can't reach Fleet.",
                    )
                }
            },
            move |s, _, result| {
                s.machine_page.addresses.saving = None;
                s.machine_page.addresses.order.remove(&id);
                match result {
                    Ok(_) => s.keep_address_order(&id, &next),
                    Err(why) => {
                        // Put back: the list shows the machine's own order again, and home is asked what it holds now.
                        s.machine_page.addresses.announcement.insert(id.clone(), "The order wasn't saved".into());
                        s.machine_page.errors.insert(id, why);
                        if let Some(link) = &s.link {
                            link.refresh_machines();
                        }
                    }
                }
            },
        );
    }

    /// The order was kept: the machine's own list takes it and the next read of home's list confirms it.
    fn keep_address_order(&mut self, key: &str, urls: &[String]) {
        self.machine_page.addresses.order.remove(key);
        if let Some(m) = self.machines.list.iter_mut().find(|m| m.id == key) {
            m.addresses.sort_by_key(|a| urls.iter().position(|u| *u == a.url).unwrap_or(usize::MAX));
        }
        if let Some(link) = &self.link {
            link.refresh_machines();
        }
    }

    /// A key on a handle: Up or Down, with Alt or without, moves the row one place (`onKey`).
    pub fn address_key(&mut self, key: &str, at: usize, up: bool, tx: &mut Tx) {
        self.move_card_address(key, at as isize, at as isize + if up { -1 } else { 1 }, tx);
    }

    /// The handle's drag moved: it begins on the first move and asks what is under the pointer.
    pub fn address_drag_move(&mut self, key: &str, from: usize, at: [f32; 2], tx: &mut Tx) {
        if self.machine_page.addresses.saving.is_some() {
            return;
        }
        let lists = &mut self.machine_page.addresses;
        if lists.drag.is_none() {
            lists.drag = Some(Dragging { key: key.to_string(), from, over: None });
        }
        let card = key.to_string();
        tx.element_at(at[0], at[1], move |s: &mut State, picked: Option<Picked>, _| {
            let prefix = format!("machine-address-{card}-");
            let over = picked.and_then(|p| p.path().rsplit('/').find_map(|seg| seg.strip_prefix(prefix.as_str())?.parse::<usize>().ok()));
            if let Some(d) = s.machine_page.addresses.drag.as_mut() {
                d.over = over;
            }
        });
    }

    /// The release: the row under the pointer takes the dragged one's place (`onDrop`); Escape drops nothing.
    pub fn address_drag_end(&mut self, cancelled: bool, tx: &mut Tx) {
        let Some(d) = self.machine_page.addresses.drag.take() else { return };
        if let (false, Some(over)) = (cancelled, d.over) {
            self.move_card_address(&d.key, d.from as isize, over as isize, tx);
        }
    }
}
