use std::time::{Duration, Instant};

use arrayvec::ArrayVec;
use cozy_chess::{Board, GameStatus, Move, Piece, Square};

use crate::{
    evaluate::{self, PIECE_VALUES},
    move_ordering::{CaptureMovesIterator, ComprehensiveMovesIterator},
    transposition_table::{NodeType, TTEntry, TranspositionTable},
};

pub const MATE_VALUE: i32 = PIECE_VALUES[Piece::King as usize];

#[derive(Debug)]
pub struct TimeControl {
    startt: Instant,
    limit: Duration,
}

impl TimeControl {
    pub fn new(limit: Duration) -> Self {
        Self {
            startt: Instant::now(),
            limit,
        }
    }

    pub fn time_up(&self) -> bool {
        self.startt.elapsed() > self.limit
    }
}

#[derive(Debug)]
pub struct SearchStats {
    pub nodes_visited: usize,
}

impl SearchStats {
    pub fn new() -> Self {
        Self { nodes_visited: 0 }
    }
}

#[derive(Debug)]
pub struct Searcher {
    max_depth: usize,
    tt: TranspositionTable,
}

impl Searcher {
    pub fn new(max_depth: usize, tt_size: usize) -> Self {
        Self {
            max_depth,
            tt: TranspositionTable::new(tt_size),
        }
    }

    pub fn search(&mut self, board: &Board, move_time: Duration) -> (SearchStats, Move, i32) {
        let mut best_move = None;
        let mut best_value = 0;

        let mut stats = SearchStats::new();
        let timer = TimeControl::new(move_time);
        for i in 1..=self.max_depth {
            let (mv, val) = self.search_internal(
                board,
                i,
                0,
                i16::MIN as i32,
                i16::MAX as i32,
                &timer,
                &mut stats,
            );

            if timer.time_up() {
                break;
            }

            best_move = mv;
            best_value = val;
        }

        (stats, best_move.unwrap(), best_value)
    }

    fn search_internal(
        &mut self,
        board: &Board,
        depth: usize,
        ply: i32,
        mut alpha: i32,
        mut beta: i32,
        timer: &TimeControl,
        stats: &mut SearchStats,
    ) -> (Option<Move>, i32) {
        stats.nodes_visited += 1;

        let alpha_orig = alpha;

        let board_hash = board.hash();
        let tt_res = self.tt.get(board_hash);
        let mut tt_move = Move {
            from: Square::A1,
            to: Square::A1,
            promotion: None,
        };
        if let Some(tte) = tt_res {
            if tte.depth >= depth {
                match tte.node_type {
                    NodeType::Exact => return (Some(tte.best_move), tte.best_value),
                    NodeType::LowerBound => {
                        beta = beta.min(tte.best_value);
                    }
                    NodeType::UpperBound => {
                        alpha = alpha.max(tte.best_value);
                    }
                }
                if alpha >= beta {
                    return (Some(tte.best_move), tte.best_value);
                }
            }

            tt_move = tte.best_move;
        }

        if board.status() == GameStatus::Won {
            return (None, -(MATE_VALUE - ply));
        } else if board.status() == GameStatus::Drawn {
            return (None, 0);
        }

        if stats.nodes_visited % 1024 == 0 && timer.time_up() {
            return (None, evaluate::evaluate(board));
        }

        if depth == 0 {
            return (None, evaluate::evaluate(board));
        }

        let mut move_buf = ArrayVec::<Move, 218>::new();
        board.generate_moves(|moves| {
            for mv in moves {
                move_buf.push(mv);

                if mv == tt_move {
                    let idx = move_buf.len() - 1;
                    move_buf.swap(0, idx);
                }
            }
            false
        });

        // let it = ComprehensiveMovesIterator::new(board, tt_move);
        let mut best_value = i16::MIN as i32;
        let mut best_move = Move { from: Square::A1, to: Square::A1, promotion: None };
        for mv in move_buf {
        // for (mv, _iscap) in it {
            let mut move_board = board.clone();
            move_board.play(mv);

            let cur_value = -self
                .search_internal(&move_board, depth - 1, ply + 1, -beta, -alpha, timer, stats)
                .1;

            if cur_value > best_value {
                best_value = cur_value;
                best_move = mv;
            }

            alpha = alpha.max(best_value);

            if alpha >= beta {
                break;
            }
        }

        let node_type = if best_value <= alpha_orig {
            NodeType::UpperBound
        } else if best_value >= beta {
            NodeType::LowerBound
        } else {
            NodeType::Exact
        };

        self.tt.set(
            board_hash,
            TTEntry {
                hash: board_hash,
                best_move,
                best_value,
                depth,
                node_type,
            },
        );

        (Some(best_move), best_value)
    }

    pub fn qsearch(
        &self,
        board: &Board,
        mut alpha: i32,
        beta: i32,
        timer: &TimeControl,
        stats: &mut SearchStats,
    ) -> i32 {
        stats.nodes_visited += 1;
        let stand_pat = evaluate::evaluate(board);
        if stats.nodes_visited % 1024 == 0 && timer.time_up() {
            return stand_pat;
        }

        if stand_pat >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat);

        // let mut move_buf = ArrayVec::<Move, 218>::new();
        // let enemy = board.colors(!board.side_to_move());
        // board.generate_moves(|mut moves| {
        //     moves.to &= enemy;
        //     for mv in moves {
        //         move_buf.push(mv);
        //     }
        //     false
        // });

        let move_buf = CaptureMovesIterator::new(board);
        for mv in move_buf {
            let mut move_board = board.clone();
            move_board.play(mv);

            let cur_value = -self.qsearch(&move_board, -beta, -alpha, timer, stats);

            alpha = alpha.max(cur_value);
            if alpha >= beta {
                return beta;
            }
        }

        alpha
    }
}
