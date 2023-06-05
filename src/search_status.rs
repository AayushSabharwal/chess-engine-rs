use cozy_chess::Move;

use crate::{utils::NULL_MOVE, types::Depth};

#[derive(Debug)]
pub struct SearchStatus {
    board_history: Vec<u64>,
    pub best_move: Move,
    pub nodes_visited: u32,
    pub ply: Depth,
}

impl SearchStatus {
    pub fn new<T>(history: T) -> Self
    where
        T: IntoIterator<Item = u64>,
    {
        let mut board_history = Vec::new();
        board_history.reserve(512);
        for i in history {
            board_history.push(i);
        }
        Self {
            board_history,
            best_move: NULL_MOVE,
            nodes_visited: 0,
            ply: 0,
        }
    }

    pub fn is_repetition_draw(&self, halfmove_count: usize, board_hash: u64) -> bool {
        if halfmove_count < 4 {
            return false;
        }
        let mut rep_count = 0;
        for &hash in self
            .board_history
            .iter()
            .rev()
            .take(halfmove_count)
            .skip(1)
            .step_by(2)
        {
            if hash == board_hash {
                rep_count += 1;
                if rep_count >= 2 {
                    return true;
                }
            }
        }
        false
    }

    pub fn push_board_hash(&mut self, board_hash: u64) {
        self.board_history.push(board_hash);
        self.ply += 1;
    }

    pub fn pop_board_hash(&mut self) {
        self.board_history.pop();
        self.ply -= 1;
    }
}

impl Default for SearchStatus {
    fn default() -> Self {
        let mut board_history = Vec::new();
        board_history.reserve(512);
        Self {
            board_history,
            best_move: NULL_MOVE,
            nodes_visited: 0,
            ply: 0,
        }
    }
}
