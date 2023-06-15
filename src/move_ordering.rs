use arrayvec::ArrayVec;
use cozy_chess::{Board, Move};

use crate::history::{HistoryTable, HISTORY_LIMIT};

pub struct MovesIterator {
    moves_evals: ArrayVec<(Move, i32, bool), 218>,
    cur: usize,
}

impl MovesIterator {
    pub fn with_all_moves(
        board: &Board,
        tt_move: Move,
        killer: Option<Move>,
        history: &HistoryTable,
    ) -> Self {
        let mut moves_evals = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|moves| {
            let src_type = board.piece_on(moves.from).unwrap();
            for mv in moves {
                // Order TT move first
                if mv == tt_move {
                    moves_evals.push((mv, i32::MAX, enemy.has(mv.to)));
                } else if enemy.has(mv.to) {
                    // Move is a capture
                    // Most Valuable Victim - Least Valuable Attacker (MVV-LVA)
                    // We prefer to take higher value pieces with lower value ones.
                    moves_evals.push((
                        mv,
                        (board.piece_on(mv.to).unwrap() as i32 * 10 - src_type as i32)
                            + i32::from(HISTORY_LIMIT),
                        true,
                    ));
                } else {
                    // Killer moves are ranked right after winning captures
                    if let Some(kmv) = killer {
                        if kmv == mv {
                            moves_evals.push((mv, i32::from(HISTORY_LIMIT), false));
                            continue;
                        }
                    }
                    // Use history for all other non-capture moves
                    moves_evals.push((mv, i32::from(history.get(board, mv)), false));
                }
            }
            false
        });

        Self {
            moves_evals,
            cur: 0,
        }
    }

    pub fn with_capture_moves(board: &Board) -> Self {
        let mut moves_evals = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|mut moves| {
            let src_type = board.piece_on(moves.from).unwrap();
            moves.to &= enemy;
            for mv in moves {
                moves_evals.push((
                    mv,
                    board.piece_on(mv.to).unwrap() as i32 * 10 - src_type as i32,
                    true,
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

impl Iterator for MovesIterator {
    type Item = (Move, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.moves_evals.len() {
            return None;
        }

        let mut best_idx = self.cur;
        let mut best_eval = self.moves_evals[self.cur].1;
        for i in (self.cur + 1)..self.moves_evals.len() {
            if self.moves_evals[i].1 > best_eval {
                best_eval = self.moves_evals[i].1;
                best_idx = i;
            }
        }

        self.moves_evals.swap(self.cur, best_idx);
        self.cur += 1;
        Some((
            self.moves_evals[self.cur - 1].0,
            self.moves_evals[self.cur - 1].2,
        ))
    }
}
