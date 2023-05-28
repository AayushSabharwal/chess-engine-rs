use std::time::{Instant, Duration};

use arrayvec::ArrayVec;
use cozy_chess::{Board, GameStatus, Move, Piece};

use crate::{
    evaluate::{self, piece_value},
    move_ordering::{self, ChecksHistoryKillers, MoveOrderer, MVVLVA, ComprehensiveMoveCollector},
    transposition_table::{TTEntry, TTNodeType, TranspositionTable}, utils::{NULL_MOVE, self},
};

const PIECE_VALUE_INF: i32 = 900 * 64;

#[derive(Debug)]
pub struct Searcher {
    max_depth: usize,
    chk_orderer: ChecksHistoryKillers,
    killer: Vec<Move>,
    history: Vec<usize>,
    tt: TranspositionTable,
}

#[derive(Debug)]
pub struct SearchStats {
    pub nodes_visited: u64,
}

impl SearchStats {
    pub fn new() -> Self {
        Self { nodes_visited: 0 }
    }
}

#[derive(Debug)]
struct TimeConstraint {
    startt: Instant,
    movetime: Duration,
}

impl TimeConstraint {
    pub fn new(movetime: Duration) -> Self {
        Self {startt: Instant::now(), movetime}
    }

    pub fn time_up(&self) -> bool {
        self.startt.elapsed() > self.movetime
    }
}

impl Searcher {
    pub fn new(max_depth: usize, ttsize: usize) -> Self {
        Self {
            max_depth,
            chk_orderer: ChecksHistoryKillers::new(),
            killer: vec![NULL_MOVE; max_depth + 1],
            history: vec![0; 64 * 12],
            tt: TranspositionTable::new(ttsize),
        }
    }

    pub fn search(
        &mut self,
        board: &Board,
        depth: Option<u32>,
        movetime: Duration,
    ) -> (SearchStats, Option<Move>, i32) {
        let mut ss = SearchStats::new();
        let mut bm: Option<Move> = None;
        let mut bv = i32::MIN;
        let d = match depth {
            Some(d) => d as usize,
            None => self.max_depth,
        };

        let timer = TimeConstraint::new(movetime);
        for i in 1..=d {
            let (bm_iter, bv_iter) =
                self.search_internal(board, i, -PIECE_VALUE_INF, PIECE_VALUE_INF, 1, &mut ss, &timer);
            if bv_iter > bv {
                bm = bm_iter;
                bv = bv_iter;
            }

            if timer.time_up() {
                return (ss, bm, bv);
            }
        }
        (ss, bm, bv)
    }

    fn search_internal(
        &mut self,
        board: &Board,
        depth: usize,
        mut alpha: i32,
        mut beta: i32,
        color: i32,
        stats: &mut SearchStats,
        timer: &TimeConstraint,
    ) -> (Option<Move>, i32) {
        let alpha_orig = alpha;
        let hash = board.hash();

        let mut ttmove = None;
        if let Some(tte) = self.tt.get_entry(hash) {
            if tte.depth >= depth {
                match tte.node_type {
                    TTNodeType::Exact => {
                        return (Some(tte.best_move), tte.best_value);
                    }
                    TTNodeType::LowerBound => {
                        alpha = alpha.max(tte.best_value);
                    }
                    TTNodeType::UpperBound => {
                        beta = beta.min(tte.best_value);
                    }
                }

                if alpha >= beta {
                    return (Some(tte.best_move), tte.best_value);
                }
            }

            ttmove = Some(tte.best_move);
        }

        if stats.nodes_visited % 1024 == 0 && timer.time_up() {
            return (None, evaluate::evaluate(board));
        }
        if depth == 0 {
            return (None, self.quiescence(board, alpha, beta, stats, timer));
        }

        stats.nodes_visited += 1;

        let status = board.status();
        if status == GameStatus::Won {
            return (None, color * -piece_value(Piece::King));
        }
        if status == GameStatus::Drawn {
            return (None, 0);
        }

        let move_collector = ComprehensiveMoveCollector::new(board, self.killer[depth], &self.history, ttmove);
        let mut best_move: Option<Move> = None;
        let mut best_value = i32::MIN;
        for (iscapture, mv) in move_collector {
            if !board.is_legal(mv) {
                panic!("{mv} ILLEGAL");
            }
            let mut move_board = board.clone();
            move_board.play_unchecked(mv);
            let cur_value = -self
                .search_internal(&move_board, depth - 1, -beta, -alpha, -color, stats, timer)
                .1;

            if cur_value > best_value {
                best_value = cur_value;
                best_move = Some(mv);
            }

            alpha = alpha.max(best_value);
            if alpha >= beta {
                if iscapture {
                    self.killer[depth] = mv;
                    self.history[utils::get_history_index(board.piece_on(mv.from).unwrap(), board.color_on(mv.from).unwrap(), mv.to)] += depth * depth;
                }
                break;
            }
        }
        // let mut captures_buffer = ArrayVec::<Move, 256>::new();
        // let mut move_buffer = ArrayVec::<Move, 256>::new();
        // let enemy = board.colors(!board.side_to_move());

        // board.generate_moves(|moves| {
        //     for mv in moves {
        //         if enemy.has(mv.to) {
        //             captures_buffer.push(mv);
        //         } else {
        //             move_buffer.push(mv);
        //         }
        //     }
        //     false
        // });

        // MVVLVA.order_moves(board, &mut captures_buffer, depth);
        // self.chk_orderer.order_moves(board, &mut move_buffer, depth);

        // let mut best_move: Option<Move> = None;
        // let mut best_value = i32::MIN;

        // for mv in captures_buffer {
        //     let mut move_board = board.clone();

        //     move_board.play_unchecked(mv);

        //     let cur_value = -self
        //         .search_internal(&move_board, depth - 1, -beta, -alpha, -color, stats, timer)
        //         .1;

        //     if cur_value > best_value {
        //         best_move = Some(mv);
        //         best_value = cur_value;
        //     }

        //     alpha = alpha.max(best_value);
        //     if alpha >= beta {
        //         break;
        //     }
        // }

        // for mv in move_buffer {
        //     let mut move_board = board.clone();

        //     move_board.play_unchecked(mv);

        //     let cur_value = -self
        //         .search_internal(&move_board, depth - 1, -beta, -alpha, -color, stats, timer)
        //         .1;

        //     if cur_value > best_value {
        //         best_move = Some(mv);
        //         best_value = cur_value;
        //     }

        //     alpha = alpha.max(best_value);
        //     if alpha >= beta {
        //         self.chk_orderer.add_to_history(board, &mv, depth);
        //         self.chk_orderer.add_to_killers(mv, depth);
        //         break;
        //     }
        // }

        let tte = TTEntry {
            hash,
            best_move: best_move.unwrap(),
            best_value,
            depth,
            node_type: if best_value <= alpha_orig {
                TTNodeType::UpperBound
            } else if best_value >= beta {
                TTNodeType::LowerBound
            } else {
                TTNodeType::Exact
            },
        };
        self.tt.set_entry(hash, tte);

        (best_move, best_value)
    }

    fn quiescence(
        &self,
        board: &Board,
        mut alpha: i32,
        beta: i32,
        stats: &mut SearchStats,
        timer: &TimeConstraint
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

        let mut capture_moves = ArrayVec::<Move, 256>::new();
        let enemy = board.colors(!board.side_to_move());

        board.generate_moves(|moves| {
            let mut captures = moves.clone();
            captures.to &= enemy;

            for mv in captures {
                capture_moves.push(mv);
            }
            false
        });

        move_ordering::MVVLVA.order_moves(board, &mut capture_moves, 0);

        for mv in capture_moves {
            let mut move_board = board.clone();
            move_board.play_unchecked(mv);

            let new_eval = -self.quiescence(&move_board, -beta, -alpha, stats, timer);

            alpha = alpha.max(new_eval);

            if alpha >= beta {
                return beta;
            }
        }

        return alpha;
    }
}
