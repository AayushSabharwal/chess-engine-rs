use cozy_chess::Move;

use crate::utils::NULL_MOVE;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TTNodeType {
    Exact,
    UpperBound,
    LowerBound,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TTEntry {
    pub best_move: Move,
    pub best_value: i32,
    pub depth: usize,
    pub node_type: TTNodeType,
}

const NULL_TTE: TTEntry = TTEntry {
    best_move: NULL_MOVE,
    best_value: -640000,
    depth: 0,
    node_type: TTNodeType::Exact,
};

#[derive(Debug)]
pub struct TranspositionTable<const N: usize> {
    table: [TTEntry; N],
}

impl<const N: usize> TranspositionTable<N> {
    pub fn new() -> Self {
        Self {
            table: [NULL_TTE; N],
        }
    }

    pub fn get_entry(&self, h: u64) -> Option<TTEntry> {
        let val = self.table[h as usize % N];

        if val == NULL_TTE {
            None
        } else {
            Some(val)
        }
    }

    pub fn set_entry(&mut self, h: u64, e: TTEntry) {
        self.table[h as usize % N] = e;
    }
}
