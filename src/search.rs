use cozy_chess::{Board, GameStatus, Move, Piece, Square};

use std::time::{Duration, Instant};

use crate::{
    evaluate::{self, PIECE_VALUES},
    move_ordering::MovesIterator,
    transposition_table::{NodeType, TTEntry, TranspositionTable},
    utils::NULL_MOVE,
};

pub const MATE_VALUE: i32 = PIECE_VALUES[Piece::King as usize];
const SCORE_INF: i32 = i16::MAX as i32;

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
pub struct SearchStatus {
    stop_search: bool,
    board_history: Vec<u64>,
    killers: [Option<Move>; 128],
    best_move: Move,
    pub nodes_visited: usize,
    ply: i32,
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
            stop_search: false,
            board_history,
            killers: [None; 128],
            best_move: NULL_MOVE,
            nodes_visited: 0,
            ply: 0,
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
        false
    }

    fn push_board_hash(&mut self, board_hash: u64) {
        self.board_history.push(board_hash);
        self.ply += 1;
    }

    fn pop_board_hash(&mut self) {
        self.board_history.pop();
        self.ply -= 1;
    }
}

#[derive(Debug)]
pub struct Searcher {
    pub tt: TranspositionTable,
}

impl Searcher {
    pub fn new(tt_size: usize) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size),
        }
    }

    pub fn search_for_time(
        &mut self,
        board: &Board,
        status: &mut SearchStatus,
        move_time: Duration,
    ) -> (Move, i32) {
        self.search(board, status, 100, move_time)
    }

    pub fn search_fixed_depth(
        &mut self,
        board: &Board,
        status: &mut SearchStatus,
        depth: usize,
    ) -> (Move, i32) {
        self.search(board, status, depth, Duration::MAX)
    }

    pub fn search(
        &mut self,
        board: &Board,
        status: &mut SearchStatus,
        max_depth: usize,
        move_time: Duration,
    ) -> (Move, i32) {
        let mut best_move = NULL_MOVE;
        let mut best_value = 0;

        let timer = TimeControl::new(move_time);

        for i in 1..=max_depth {
            let val = if i < 5 {
                self.search_internal(board, status, i, -SCORE_INF, SCORE_INF, &timer)
            } else {
                let mut window_size = 20;
                let mut alpha = best_value - window_size;
                let mut beta = best_value + window_size;
                let mut tmp_val;
                loop {
                    tmp_val = self.search_internal(board, status, i, alpha, beta, &timer);
                    if tmp_val >= beta {
                        beta = beta.saturating_add(window_size);
                        window_size = window_size.saturating_mul(2);
                    } else if tmp_val <= alpha {
                        alpha = alpha.saturating_sub(window_size);
                        window_size = window_size.saturating_mul(2);
                    } else {
                        break;
                    }
                }
                tmp_val
            };

            if status.stop_search || timer.time_up() {
                break;
            }

            best_move = status.best_move;
            best_value = val;
        }

        (best_move, best_value)
    }

    fn search_internal(
        &mut self,
        board: &Board,
        status: &mut SearchStatus,
        depth: usize,
        mut alpha: i32,
        mut beta: i32,
        timer: &TimeControl,
    ) -> i32 {
        status.nodes_visited += 1;

        if status.stop_search || status.nodes_visited % 1024 == 0 && timer.time_up() {
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
            if status.ply > 0 && tte.depth >= depth as u8 {
                match tte.node_type {
                    NodeType::Exact => {
                        return tte.best_value as i32;
                    }
                    NodeType::LowerBound => {
                        alpha = alpha.max(tte.best_value as i32);
                    }
                    NodeType::UpperBound => {
                        beta = beta.min(tte.best_value as i32);
                    }
                }
                if alpha >= beta {
                    return tte.best_value as i32;
                }
            }

            tt_move = tte.best_move;
        }

        if board.status() == GameStatus::Won {
            return -(MATE_VALUE - status.ply);
        } else if board.status() == GameStatus::Drawn {
            return 0;
        }

        if depth == 0 {
            return qsearch(board, alpha, beta, timer, status);
        }

        let it = MovesIterator::with_all_moves(board, tt_move, status.killers[depth]);
        let mut best_value = i16::MIN as i32;
        let mut best_move = Move {
            from: Square::A1,
            to: Square::A1,
            promotion: None,
        };
        let mut first_move = false;
        status.push_board_hash(board_hash);

        for (mv, iscapture) in it {
            let mut move_board = board.clone();
            move_board.play(mv);

            let cur_value = if first_move {
                first_move = false;
                -self.search_internal(&move_board, status, depth - 1, -beta, -alpha, timer)
            } else {
                let tmp_value = -self.search_internal(
                    &move_board,
                    status,
                    depth - 1,
                    -alpha - 1,
                    -alpha,
                    timer,
                );
                if alpha < tmp_value && tmp_value < beta {
                    -self.search_internal(&move_board, status, depth - 1, -beta, -alpha, timer)
                } else {
                    tmp_value
                }
            };

            if cur_value > best_value {
                best_value = cur_value;
                best_move = mv;
            }

            alpha = alpha.max(best_value);

            if alpha >= beta {
                if !iscapture {
                    status.killers[depth] = Some(mv);
                }

                break;
            }
        }

        status.pop_board_hash();

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
                best_value: best_value as i16,
                depth: depth as u8,
                node_type,
            },
        );

        if status.ply == 0 {
            status.best_move = best_move;
        }

        best_value
    }
}

fn qsearch(
    board: &Board,
    mut alpha: i32,
    beta: i32,
    timer: &TimeControl,
    status: &mut SearchStatus,
) -> i32 {
    status.nodes_visited += 1;
    if status.nodes_visited % 1024 == 0 && timer.time_up() {
        return 0;
    }

    let stand_pat = evaluate::evaluate(board);
    if stand_pat >= beta {
        return stand_pat;
    }
    alpha = alpha.max(stand_pat);

    let move_buf = MovesIterator::with_capture_moves(board);
    let mut best_value = stand_pat;
    for (mv, _) in move_buf {
        let mut move_board = board.clone();
        move_board.play(mv);

        let cur_value = -qsearch(&move_board, -beta, -alpha, timer, status);

        best_value = best_value.max(cur_value);

        alpha = alpha.max(cur_value);
        if alpha >= beta {
            return alpha;
        }
    }

    best_value
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use arrayvec::ArrayVec;
    use cozy_chess::{Board, Move};

    use crate::search::{TimeControl, SCORE_INF};

    use super::{SearchStatus, Searcher};

    #[test]
    fn repetition_draw_check() {
        let mut board = Board::from_fen(
            "rnbqkb1r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQKB1R w - - 0 1",
            false,
        )
        .unwrap();
        let mut board_history: ArrayVec<u64, 128> = ArrayVec::new();
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
        let bv = Searcher::new(10_000_000).search_internal(
            &board,
            &mut status,
            100,
            -SCORE_INF,
            SCORE_INF,
            &timer,
        );
        assert_eq!(bv, 0);
    }

    #[test]
    fn force_repetition() {
        let board = Board::from_fen("7k/5pp1/6p1/8/1rn3Q1/qrb5/8/3K4 w - - 0 1", false).unwrap();
        let (bm, bv) = Searcher::new(10_000_000).search_for_time(
            &board,
            &mut SearchStatus::new(std::iter::empty()),
            Duration::from_secs(10),
        );
        assert!(bm == "g4h4".parse::<Move>().unwrap() || bm == "g4c8".parse::<Move>().unwrap());
        assert_eq!(bv, 0);
    }
}
