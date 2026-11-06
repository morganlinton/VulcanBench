//! The undo journal: a stack of transaction frames, each a list of inverses.

use std::collections::HashMap;

/// The store's data map, handed to every inverse when it runs.
pub type Data = HashMap<String, String>;

/// A one-shot inverse operation: applied to the data map, it reverses one
/// mutation exactly.
pub type Undo = Box<dyn FnOnce(&mut Data)>;

/// Transaction frames, innermost last.
#[derive(Default)]
pub struct UndoJournal {
    frames: Vec<Vec<Undo>>,
}

impl UndoJournal {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if at least one frame is open.
    pub fn in_transaction(&self) -> bool {
        todo!()
    }

    /// Open a new (nested) frame.
    pub fn push(&mut self) {
        todo!()
    }

    /// Record an inverse into the current frame. With no frame open the
    /// mutation is durable (autocommit) and the inverse is dropped.
    pub fn record(&mut self, undo: Undo) {
        let _ = undo;
        todo!()
    }

    /// Undo the current frame: run its inverses against `data` in reverse
    /// order, then drop the frame. Returns `false` if no frame is open.
    pub fn rollback(&mut self, data: &mut Data) -> bool {
        let _ = data;
        todo!()
    }

    /// Close the current frame. Its inverses are folded into the parent frame
    /// (order preserved) so a later parent rollback still undoes them; at the
    /// top level there is no parent, so the changes become durable. Returns
    /// `false` if no frame is open.
    pub fn commit(&mut self) -> bool {
        todo!()
    }

    /// How many frames are open.
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}
