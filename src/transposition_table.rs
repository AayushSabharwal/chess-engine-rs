use std::mem::size_of;

use cozy_chess::Move;

#[derive(Debug, Copy, Clone)]
pub enum NodeType {
    Exact,
    UpperBound,
    LowerBound,
}

#[derive(Debug, Copy, Clone)]
pub struct TTEntry {
    pub hash: u64,
    pub best_move: Move,
    pub best_value: i16,
    pub depth: u8,
    pub node_type: NodeType,
}

#[derive(Debug)]
pub struct TranspositionTable {
    buffer: Vec<Option<TTEntry>>,
}

impl TranspositionTable {
    pub fn new(bytes: usize) -> Self {
        Self {
            buffer: vec![None; bytes_to_entries(bytes)],
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn get(&self, hash: u64) -> Option<TTEntry> {
        let idx = hash as usize % self.buffer.len();
        self.buffer[idx].filter(|&tte| tte.hash == hash)
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn set(&mut self, hash: u64, value: TTEntry) {
        let idx = hash as usize % self.buffer.len();
        self.buffer[idx] = Some(value);
    }

    pub fn clear(&mut self) {
        for i in 0..self.buffer.len() {
            self.buffer[i] = None;
        }
    }
}

const fn bytes_to_entries(bytes: usize) -> usize {
    bytes / size_of::<Option<TTEntry>>()
}
