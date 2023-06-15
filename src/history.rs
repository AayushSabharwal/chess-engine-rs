use cozy_chess::{Board, Move};

use crate::types::Depth;

pub const HISTORY_LIMIT: i16 = i16::MAX / 2;

#[derive(Debug)]
pub struct HistoryTable {
    table: [i16; 12 * 64],
}

impl HistoryTable {
    pub const fn new() -> Self {
        Self {
            table: [0; 12 * 64],
        }
    }

    pub fn get(&self, board: &Board, mv: Move) -> i16 {
        self.table[history_index(board, mv)]
    }

    pub fn get_mut(&mut self, board: &Board, mv: Move) -> &mut i16 {
        &mut self.table[history_index(board, mv)]
    }

    pub fn update(&mut self, board: &Board, mv: Move, depth: Depth) {
        let entry = self.get_mut(board, mv);
        let delta = history_delta(i16::from(depth));
        *entry += delta;
        if *entry >= HISTORY_LIMIT {
            self.normalize();
        }
    }

    pub fn normalize(&mut self) {
        for x in self.table.iter_mut() {
            *x /= 2;
        }
    }

    pub fn clear(&mut self) {
        self.table.fill(0);
    }
}

pub const fn history_delta(depth: i16) -> i16 {
    depth * depth + depth
}

pub fn history_index(board: &Board, mv: Move) -> usize {
    (board.color_on(mv.from).unwrap() as usize * 6 + board.piece_on(mv.from).unwrap() as usize) * 64
        + mv.to as usize
}
