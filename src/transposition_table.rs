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
    pub hash: u64,
    pub best_move: Move,
    pub best_value: i32,
    pub depth: usize,
    pub node_type: TTNodeType,
}

const NULL_TTE: TTEntry = TTEntry {
    hash: 0,
    best_move: NULL_MOVE,
    best_value: -640000,
    depth: 0,
    node_type: TTNodeType::Exact,
};

#[derive(Debug)]
pub struct TranspositionTable {
    table: Vec<TTEntry>,
    size: usize
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        Self {
            table: vec![NULL_TTE; size],
            size,
        }
    }

    pub fn get_entry(&self, h: u64) -> Option<TTEntry> {
        let val = self.table[h as usize % self.size];

        if val != NULL_TTE && val.hash != h {
            None
        } else {
            Some(val)
        }
    }

    #[inline]
    pub fn set_entry(&mut self, h: u64, e: TTEntry) {
        self.table[h as usize % self.size] = e;
    }
}
