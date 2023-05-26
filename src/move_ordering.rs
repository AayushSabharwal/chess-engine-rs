use arrayvec::ArrayVec;
use cozy_chess::{Board, Move};

use crate::evaluate;
use crate::utils::{piece_to_index, NULL_MOVE};

pub trait MoveOrderer {
    fn order_moves(&self, board: &Board, moves: &mut ArrayVec<Move, 256>, depth: usize);
}

pub struct MVVLVA;

impl MoveOrderer for MVVLVA {
    fn order_moves(&self, board: &Board, moves: &mut ArrayVec<Move, 256>, _depth: usize) {
        moves.sort_unstable_by_key(|mv| {
            evaluate::piece_value(board.piece_on(mv.from).unwrap())
                - evaluate::piece_value(board.piece_on(mv.to).unwrap())
        });
    }
}

#[derive(Debug)]
pub struct ChecksHistoryKillers {
    pub history: Vec<usize>,
    pub killers: Vec<Move>,
}

impl ChecksHistoryKillers {
    pub fn new() -> Self {
        Self {
            history: vec![0; 64 * 12],
            killers: vec![NULL_MOVE; 50],
        }
    }

    fn move_to_history_index(board: &Board, mv: &Move) -> usize {
        piece_to_index(
            board.piece_on(mv.from).unwrap(),
            board.color_on(mv.from).unwrap(),
        ) * 64
            + mv.to as usize
    }

    pub fn add_to_history(&mut self, board: &Board, mv: &Move, depth: usize) {
        self.history[Self::move_to_history_index(board, mv)] += depth * depth;
    }

    pub fn add_to_killers(&mut self, mv: Move, depth: usize) {
        self.killers[depth] = mv;
    }
}

impl MoveOrderer for ChecksHistoryKillers {
    fn order_moves(&self, board: &Board, moves: &mut ArrayVec<Move, 256>, depth: usize) {
        let mut sidx = 0;
        for i in 0..moves.len() {
            let mv = moves[i];

            if mv == self.killers[depth] {
                moves.swap(i, sidx);
                moves.swap(sidx, 0);
                sidx += 1;
                continue;
            }

            let mut move_board = board.clone();
            move_board.play_unchecked(mv);
            if move_board.checkers().0 > 0 {
                moves.swap(i, sidx);
                sidx += 1;
            }
        }

        if sidx < moves.len() {
            moves[sidx..].sort_unstable_by_key(|mv| Self::move_to_history_index(board, mv));
        }
    }
}
