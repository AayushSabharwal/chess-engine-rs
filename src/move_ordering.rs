use arrayvec::ArrayVec;
use cozy_chess::{Board, Move};

use crate::evaluate::PIECE_VALUES;

pub struct CaptureMovesIterator {
    moves_evals: ArrayVec<(Move, (u16, u16)), 218>,
    cur: usize,
}

impl CaptureMovesIterator {
    pub fn new(board: &Board) -> Self {
        let mut moves_evals = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|mut moves| {
            moves.to &= enemy;
            let src_eval = PIECE_VALUES[board.piece_on(moves.from).unwrap() as usize] as u16;
            for mv in moves {
                moves_evals.push((
                    mv,
                    (
                        PIECE_VALUES[board.piece_on(mv.to).unwrap() as usize] as u16,
                        src_eval,
                    ),
                ));
            }
            false
        });

        Self {
            moves_evals,
            cur: 0,
        }
    }
}

impl Iterator for CaptureMovesIterator {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.moves_evals.len() {
            return None;
        }

        let mut best_idx = self.cur;
        let mut best_eval = (0, 0);
        for i in self.cur + 1..self.moves_evals.len() {
            if self.moves_evals[i].1 > best_eval {
                best_eval = self.moves_evals[i].1;
                best_idx = i;
            }
        }

        self.moves_evals.swap(self.cur, best_idx);
        self.cur += 1;
        Some(self.moves_evals[best_idx].0)
    }
}

pub struct ComprehensiveMovesIterator {
    moves_evals: ArrayVec<(Move, i32, bool), 218>,
    cur: usize,
}

impl ComprehensiveMovesIterator {
    pub fn new(board: &Board, tt_move: Move) -> Self {
        let mut moves_evals = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|moves| {
            let src_type = board.piece_on(moves.from).unwrap();
            let src_eval = PIECE_VALUES[src_type as usize];
            for mv in moves {
                if mv == tt_move {
                    moves_evals.push((mv, i16::MAX as i32, enemy.has(mv.to)));
                }
                else if enemy.has(mv.to) {
                    moves_evals.push((mv, PIECE_VALUES[board.piece_on(mv.to).unwrap() as usize] * 10 + src_eval, true));
                }
                else {
                    moves_evals.push((mv, 0, false));
                }
            }
            false
        });

        Self {
            moves_evals,
            cur: 0,
        }
    }
}

impl Iterator for ComprehensiveMovesIterator {
    type Item = (Move, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.moves_evals.len() {
            return None;
        }

        let mut best_idx = self.cur;
        let mut best_eval = 0;
        for i in self.cur + 1..self.moves_evals.len() {
            if self.moves_evals[i].1 > best_eval {
                best_eval = self.moves_evals[i].1;
                best_idx = i;
            }
        }

        self.moves_evals.swap(self.cur, best_idx);
        self.cur += 1;
        Some((self.moves_evals[best_idx].0, self.moves_evals[best_idx].2))
    }
}
