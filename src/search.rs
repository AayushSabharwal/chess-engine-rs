use std::time::{Duration, Instant};

use arrayvec::ArrayVec;
use cozy_chess::{Board, GameStatus, Move, Piece, Square};

use crate::{
    evaluate::{self, PIECE_VALUES},
    move_ordering::MovesIterator,
    transposition_table::{NodeType, TTEntry, TranspositionTable},
    utils::NULL_MOVE,
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

struct SearchStatus {
    stop_search: bool,
    board_history: ArrayVec<u64, 128>,
    best_move: Move,
}

impl SearchStatus {
    fn new(board_history: ArrayVec<u64, 128>) -> Self {
        Self {
            stop_search: false,
            board_history,
            best_move: NULL_MOVE,
        }
    }

    fn is_repetition_draw(&self, halfmove_count: usize, board_hash: u64) -> bool {
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
        return false;
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

    pub fn search(
        &mut self,
        board: &Board,
        board_history: ArrayVec<u64, 128>,
        move_time: Duration,
    ) -> (SearchStats, Move, i32) {
        let mut best_move = NULL_MOVE;
        let mut best_value = 0;

        let mut stats = SearchStats::new();
        let timer = TimeControl::new(move_time);
        let mut status = SearchStatus::new(board_history);

        for i in 1..=self.max_depth {
            let val = self.search_internal(
                board,
                &mut status,
                i,
                0,
                i16::MIN as i32,
                i16::MAX as i32,
                &timer,
                &mut stats,
            );

            if status.stop_search || timer.time_up() {
                break;
            }

            best_move = status.best_move;
            best_value = val;
        }

        (stats, best_move, best_value)
    }

    fn search_internal(
        &mut self,
        board: &Board,
        status: &mut SearchStatus,
        depth: usize,
        ply: i32,
        mut alpha: i32,
        mut beta: i32,
        timer: &TimeControl,
        stats: &mut SearchStats,
    ) -> i32 {
        stats.nodes_visited += 1;

        if status.stop_search || stats.nodes_visited % 1024 == 0 && timer.time_up() {
            status.stop_search = true;
            return 0;
        }

        let alpha_orig = alpha;

        let board_hash = board.hash();

        if status.is_repetition_draw(board.halfmove_clock() as usize, board_hash) {
            return 0;
        }

        let tt_res = self.tt.get(board_hash);
        let mut tt_move = NULL_MOVE;

        if let Some(tte) = tt_res {
            if tte.depth >= depth {
                match tte.node_type {
                    NodeType::Exact => {
                        if ply == 0 {
                            status.best_move = tte.best_move;
                        }
                        return tte.best_value;
                    }
                    NodeType::LowerBound => {
                        beta = beta.min(tte.best_value);
                    }
                    NodeType::UpperBound => {
                        alpha = alpha.max(tte.best_value);
                    }
                }
                if alpha >= beta {
                    if ply == 0 {
                        status.best_move = tte.best_move;
                    }
                    return tte.best_value;
                }
            }

            tt_move = tte.best_move;
        }

        if board.status() == GameStatus::Won {
            return -(MATE_VALUE - ply);
        } else if board.status() == GameStatus::Drawn {
            return 0;
        }

        if depth == 0 {
            return evaluate::evaluate(board);
        }

        let it = MovesIterator::with_all_moves(board, tt_move);
        let mut best_value = i16::MIN as i32;
        let mut best_move = Move {
            from: Square::A1,
            to: Square::A1,
            promotion: None,
        };
        status.board_history.push(board_hash);

        for (mv, _iscap) in it {
            let mut move_board = board.clone();
            move_board.play(mv);

            let cur_value = -self.search_internal(
                &move_board,
                status,
                depth - 1,
                ply + 1,
                -beta,
                -alpha,
                timer,
                stats,
            );

            if cur_value > best_value {
                best_value = cur_value;
                best_move = mv;
            }

            alpha = alpha.max(best_value);

            if alpha >= beta {
                break;
            }
        }

        status.board_history.pop();

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

        if ply == 0 {
            status.best_move = best_move;
        }

        best_value
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

        let move_buf = MovesIterator::with_capture_moves(board);
        for (mv, _) in move_buf {
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

#[cfg(test)]
mod test {
    use std::time::Duration;

    use arrayvec::ArrayVec;
    use cozy_chess::{Board, Move};

    use crate::search::{SearchStats, TimeControl};

    use super::{SearchStatus, Searcher};

    #[test]
    fn repetition_draw_check() {
        let mut board = Board::from_fen(
            "rnbqkb1r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQKB1R w - - 0 1",
            false,
        )
        .unwrap();
        let mut board_history = ArrayVec::new();
        board_history.push(board.hash());
        let moves = [
            "h1g1", "h8g8", "g1h1", "g8h8", "h1g1", "h8g8", "g1h1", "g8h8",
        ];

        for mv in moves {
            board.play_unchecked(mv.parse::<Move>().unwrap());
            board_history.push(board.hash());
        }
        board_history.pop();

        let mut status = SearchStatus::new(board_history);
        let timer = TimeControl::new(Duration::from_secs(1));
        let bv = Searcher::new(1000, 1000).search_internal(
            &board,
            &mut status,
            100,
            0,
            i32::MIN,
            i32::MAX,
            &timer,
            &mut SearchStats::new(),
        );
        assert_eq!(bv, 0);
    }

    #[test]
    fn force_repetition() {
        let board = Board::from_fen("7k/5pp1/6p1/8/1rn3Q1/qrb5/8/3K4 w - - 0 1", false).unwrap();
        let (_, bm, bv) =
            Searcher::new(100, 100000).search(&board, ArrayVec::new(), Duration::from_secs(10));
        assert!(bm == "g4h4".parse::<Move>().unwrap() || bm == "g4c8".parse::<Move>().unwrap());
        assert_eq!(bv, 0);
    }
}
