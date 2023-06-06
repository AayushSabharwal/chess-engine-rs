use cozy_chess::{Board, GameStatus, Move, Piece};

use std::time::{Duration, Instant};

use crate::{
    evaluate::{self, PIECE_VALUES},
    history::HistoryTable,
    move_ordering::MovesIterator,
    transposition_table::{NodeType, TTEntry, TranspositionTable},
    types::{Depth, Value},
    utils::NULL_MOVE,
};

pub const MATE_VALUE: Value = PIECE_VALUES[Piece::King as usize];
const SCORE_INF: Value = Value::MAX;

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
    pub nodes_visited: u32,
    pub depth: u8,
}

impl Default for SearchStats {
    fn default() -> Self {
        Self {
            nodes_visited: 0,
            depth: 0,
        }
    }
}

#[derive(Debug)]
pub struct Searcher {
    pub tt: TranspositionTable,
    board_history: Vec<u64>,
    stop_search: bool,
    history: HistoryTable,
    killers: [Option<Move>; 128],
    best_move: Move,
    ply: u8,
}

impl Searcher {
    pub fn new(tt_size: usize) -> Self {
        let mut board_history = Vec::new();
        board_history.reserve(512);
        Self {
            tt: TranspositionTable::new(tt_size),
            board_history,
            stop_search: false,
            history: HistoryTable::new(),
            killers: [None; 128],
            best_move: NULL_MOVE,
            ply: 0,
        }
    }

    pub fn new_game(&mut self) {
        self.tt.clear();
    }

    pub fn search_for_time(
        &mut self,
        board: &mut Board,
        moves: &Vec<Move>,
        stats: &mut SearchStats,
        move_time: Duration,
    ) -> (Move, Value) {
        self.search(board, moves, stats, 128, move_time)
    }

    pub fn search_fixed_depth(
        &mut self,
        board: &mut Board,
        moves: &Vec<Move>,
        stats: &mut SearchStats,
        depth: Depth,
    ) -> (Move, Value) {
        self.search(board, moves, stats, depth, Duration::MAX)
    }

    pub fn search(
        &mut self,
        board: &mut Board,
        moves: &Vec<Move>,
        stats: &mut SearchStats,
        max_depth: Depth,
        move_time: Duration,
    ) -> (Move, Value) {
        let mut best_move = NULL_MOVE;
        let mut best_value = 0;

        let timer = TimeControl::new(move_time);
        self.search_reset(board, moves);

        for i in 1..=max_depth {
            let val = if i < 5 {
                self.search_internal(board, stats, i, -SCORE_INF, SCORE_INF, &timer)
            } else {
                let mut window_size = 20;
                let mut alpha = best_value - window_size;
                let mut beta = best_value + window_size;
                let mut tmp_val;
                loop {
                    tmp_val = self.search_internal(board, stats, i, alpha, beta, &timer);
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

            self.history.normalize();
            if self.stop_search || timer.time_up() {
                break;
            }

            stats.depth = i;
            best_move = self.best_move;
            best_value = val;
        }

        (best_move, best_value)
    }

    fn search_reset(&mut self, board: &mut Board, moves: &Vec<Move>) {
        self.stop_search = false;
        self.history.clear();
        self.killers.fill(None);

        self.board_history.clear();
        self.board_history.push(board.hash());

        for &mv in moves {
            board.play_unchecked(mv);
            self.board_history.push(board.hash());
        }
        self.board_history.pop();

        self.best_move = NULL_MOVE;
        self.ply = 0;
    }

    fn search_internal(
        &mut self,
        board: &Board,
        stats: &mut SearchStats,
        depth: Depth,
        mut alpha: Value,
        mut beta: Value,
        timer: &TimeControl,
    ) -> Value {
        stats.nodes_visited += 1;

        if self.stop_search || stats.nodes_visited % 1024 == 0 && timer.time_up() {
            self.stop_search = true;
            return 0;
        }

        let alpha_orig = alpha;
        let board_hash = board.hash();
        let is_pv_node = (beta - alpha) != 1;

        if self.is_repetition_draw(board.halfmove_clock() as usize, board_hash) {
            return 0;
        }

        let tt_res = self.tt.get(board_hash);
        let mut tt_move = NULL_MOVE;

        if let Some(tte) = tt_res {
            if self.ply > 0 && tte.depth >= depth {
                match tte.node_type {
                    NodeType::Exact => {
                        return tte.best_value;
                    }
                    NodeType::LowerBound => {
                        alpha = alpha.max(tte.best_value);
                    }
                    NodeType::UpperBound => {
                        beta = beta.min(tte.best_value);
                    }
                }
                if alpha >= beta {
                    return tte.best_value;
                }
            }

            tt_move = tte.best_move;
        }

        if board.status() == GameStatus::Won {
            return -(MATE_VALUE - Value::from(self.ply));
        } else if board.status() == GameStatus::Drawn {
            return 0;
        }

        if depth == 0 {
            return qsearch(board, alpha, beta, timer, stats);
        }

        let it = MovesIterator::with_all_moves(
            board,
            tt_move,
            self.killers[usize::from(depth)],
            &self.history,
        );
        let mut best_value = -SCORE_INF;
        let mut best_move = NULL_MOVE;
        let mut first_move = false;
        self.push_board_hash(board_hash);

        if !is_pv_node && depth >= 3 {
            let null_move = board.null_move();
            if let Some(move_board) = null_move {
                let null_move_value =
                    -self.search_internal(&move_board, stats, depth - 3, -beta, -beta + 1, timer);
                if null_move_value >= beta {
                    self.pop_board_hash();
                    return null_move_value;
                }
            }
        }

        for (mv, iscapture) in it {
            let mut move_board = board.clone();
            move_board.play(mv);

            let cur_value = if first_move {
                first_move = false;
                -self.search_internal(&move_board, stats, depth - 1, -beta, -alpha, timer)
            } else {
                let tmp_value =
                    -self.search_internal(&move_board, stats, depth - 1, -alpha - 1, -alpha, timer);
                if alpha < tmp_value && tmp_value < beta {
                    -self.search_internal(&move_board, stats, depth - 1, -beta, -alpha, timer)
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
                    self.killers[usize::from(depth)] = Some(mv);
                    self.history.update(board, mv, depth);
                }

                break;
            }
        }

        self.pop_board_hash();

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

        if self.ply == 0 {
            self.best_move = best_move;
        }

        best_value
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

fn qsearch(
    board: &Board,
    mut alpha: Value,
    beta: Value,
    timer: &TimeControl,
    stats: &mut SearchStats,
) -> Value {
    stats.nodes_visited += 1;
    if stats.nodes_visited % 1024 == 0 && timer.time_up() {
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

        let cur_value = -qsearch(&move_board, -beta, -alpha, timer, stats);

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

    use crate::search::{SearchStats, TimeControl, SCORE_INF};

    use super::Searcher;

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
        ]
        .iter()
        .map(|&mv| mv.parse::<Move>().unwrap())
        .collect::<Vec<Move>>();

        let mut stats = SearchStats::default();
        let timer = TimeControl::new(Duration::from_secs(1));
        let bv = Searcher::new(10_000_000)
            .search_internal(&mut board, &mut stats, 100, -SCORE_INF, SCORE_INF, &timer);
        assert_eq!(bv, 0);
    }

    #[test]
    fn force_repetition() {
        let board = Board::from_fen("7k/5pp1/6p1/8/1rn3Q1/qrb5/8/3K4 w - - 0 1", false).unwrap();
        let (bm, bv) = Searcher::new(10_000_000).search_for_time(
            &mut board,
            &Vec::new(),
            &mut SearchStats::default(),
            Duration::from_secs(10),
        );
        assert!(bm == "g4h4".parse::<Move>().unwrap() || bm == "g4c8".parse::<Move>().unwrap());
        assert_eq!(bv, 0);
    }
}
