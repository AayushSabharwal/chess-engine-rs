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
    pub best_value: i32,
    pub depth: usize,
    pub node_type: NodeType,
}

#[derive(Debug)]
pub struct TranspositionTable {
    buffer: Vec<Option<TTEntry>>,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![None; size],
        }
    }

    pub fn get(&self, hash: u64) -> Option<TTEntry> {
        let idx = hash as usize % self.buffer.len();
        if let Some(tte) = self.buffer[idx] {
            if tte.hash == hash {
                Some(tte)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn set(&mut self, hash: u64, value: TTEntry) {
        let idx = hash as usize % self.buffer.len();
        self.buffer[idx] = Some(value);
    }

    pub fn resize(&mut self, size: usize) {
        self.buffer = vec![None; size];
    }
}
