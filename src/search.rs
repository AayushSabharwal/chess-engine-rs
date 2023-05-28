use std::time::{Duration, Instant};

use arrayvec::ArrayVec;
use cozy_chess::{Board, GameStatus, Move, Piece};

use crate::evaluate::{self, PIECE_VALUES};

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
}

impl Searcher {
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }

    pub fn search(&self, board: &Board, move_time: Duration) -> (SearchStats, Move, i32) {
        let mut best_move = None;
        let mut best_value = 0;

        let mut stats = SearchStats::new();
        let timer = TimeControl::new(move_time);
        for i in 1..=self.max_depth {
            let (mv, val) = self.search_internal(board, i, i16::MIN as i32, i16::MAX as i32, 1, &timer, &mut stats);

            if timer.time_up() {
                break;
            }
            eprintln!("{:?} {}", mv, val);
            best_move = mv;
            best_value = val;
        }

        (stats, best_move.unwrap(), best_value)
    }

    fn search_internal(
        &self,
        board: &Board,
        depth: usize,
        mut alpha: i32,
        beta: i32,
        color: i32,
        timer: &TimeControl,
        stats: &mut SearchStats,
    ) -> (Option<Move>, i32) {
        stats.nodes_visited += 1;

        if stats.nodes_visited % 1024 == 0 && timer.time_up() {
            return (None, evaluate::evaluate(board));
        }

        if depth == 0 {
            return (None, self.qsearch(board, alpha, beta, timer, stats));
        }

        if board.status() == GameStatus::Won {
            return (None, color * -PIECE_VALUES[Piece::King as usize]);
        } else if board.status() == GameStatus::Drawn {
            return (None, 0);
        }

        let mut move_buf = ArrayVec::<Move, 218>::new();
        board.generate_moves(|moves| {
            for mv in moves {
                move_buf.push(mv);
            }
            false
        });

        let mut best_value = i16::MIN as i32;
        let mut best_move = None;
        for mv in move_buf {
            let mut move_board = board.clone();
            move_board.play_unchecked(mv);

            let cur_value = -self
                .search_internal(&move_board, depth - 1, -beta, -alpha, -color, timer, stats)
                .1;

            if cur_value > best_value {
                best_value = cur_value;
                best_move = Some(mv);
            }

            alpha = alpha.max(best_value);

            if alpha >= beta {
                break;
            }
        }

        (best_move, best_value)
    }

    pub fn qsearch(&self, board: &Board, mut alpha: i32, beta: i32, timer: &TimeControl, stats: &mut SearchStats) -> i32 {
        let stand_pat = evaluate::evaluate(board);
        if stats.nodes_visited % 1024 == 0 && timer.time_up() {
            return stand_pat;
        }

        if stand_pat >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat);

        let mut move_buf = ArrayVec::<Move, 218>::new();
        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|mut moves| {
            moves.to &= enemy;
            for mv in moves {
                move_buf.push(mv);
            }
            false
        });

        for mv in move_buf {
            let mut move_board = board.clone();
            move_board.play_unchecked(mv);

            let cur_value = -self.qsearch(&move_board, -beta, -alpha, timer, stats);

            alpha = alpha.max(cur_value);
            if alpha >= beta {
                return beta;
            }
        }

        alpha
    }
}
