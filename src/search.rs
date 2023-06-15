use cozy_chess::{Board, GameStatus, Move, Piece};

use std::time::{Duration, Instant};

use crate::{
    evaluate::{self, PIECE_VALUES},
    history::HistoryTable,
    lmr_table::LMRTable,
    move_ordering::MovesIterator,
    transposition_table::{NodeType, TTEntry, TranspositionTable},
    types::{Depth, Value},
    utils::{uci_to_kxr_move, NULL_MOVE},
};

pub const MATE_VALUE: Value = PIECE_VALUES[Piece::King as usize];
const SCORE_INF: Value = Value::MAX;
const LMR_MIN_DEPTH: Depth = 3;
const RFP_EVAL_MARGIN: Value = 75;

// To end searches early
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

#[derive(Debug, Default)]
pub struct SearchStats {
    pub nodes_visited: u32,
    pub depth: u8,
}

#[derive(Debug)]
pub struct Searcher {
    pub tt: TranspositionTable,
    board_history: Vec<u64>,
    stop_search: bool,
    history: HistoryTable,
    killers: [Option<Move>; 257],
    lmr_table: LMRTable,
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
            killers: [None; 257],
            lmr_table: LMRTable::new(),
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
        self.search(board, moves, stats, Depth::MAX, move_time)
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

        // Iterative Deepening (ID)
        // Searching to a lower depth allows us to order moves better, so that higher depth searches
        // get more cutoffs. Number of nodes increases exponentially with depth, so smaller searches
        // are significantly cheaper.
        for i in 1..=max_depth {
            let val = if i < 5 {
                self.search_internal(board, stats, i, -SCORE_INF, SCORE_INF, &timer)
            } else {
                // Aspiration windows
                // After a few shallow searches, instead of starting alpha/beta at -inf,inf use the
                // previous score as an estimate. If the returned score is out of the range we
                // expected it to be, search again after increasing bounds. Since the bounds
                // increase exponentially, we don't have to research much and searches with smaller
                // bounds complete much quicker due to easier cutoffs.
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
            // Only use results from a fully completed search
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

        // Board history keeps track of past Zobrist hashes, which is used for repetition draw
        // checks
        for &mv in moves {
            let mut mv = mv;
            uci_to_kxr_move(board, &mut mv);
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

        // If the search has timed out, ensure everyone knows about it and stop
        // searching
        if self.stop_search || stats.nodes_visited % 1024 == 0 && timer.time_up() {
            self.stop_search = true;
            return 0;
        }

        let alpha_orig = alpha;
        let board_hash = board.hash();
        // PV nodes are not searched with a null window
        // TODO: Consider making this a const generic
        let is_pv_node = beta > alpha + 1;

        // Draw Detection
        // If the engine can detect repetition draws, it can force a draw from a losing position
        // and avoid draws from winning positions.
        if self.is_repetition_draw(board.halfmove_clock() as usize, board_hash) {
            return 0;
        }

        // Transposition Table
        // Uses Zobrist hashes to store the results of past searches from the same position.
        // This allows us to save considerable work.
        let tt_res = self.tt.get(board_hash);
        let mut tt_move = NULL_MOVE;
        let static_eval;

        if let Some(tte) = tt_res {
            // Don't use TT at the root, and don't use it if it wasn't searched deeper than
            // we'll search this position.
            if self.ply > 0 && tte.depth >= depth {
                match tte.node_type {
                    // If the node obtained an exact value for this position, just use it
                    NodeType::Exact => {
                        return tte.best_value;
                    }
                    // If the node obtained a lower bound on the value, use that to update ours
                    NodeType::LowerBound => {
                        alpha = alpha.max(tte.best_value);
                    }
                    // Similarly for upper bound
                    NodeType::UpperBound => {
                        beta = beta.min(tte.best_value);
                    }
                }
                // In case updating the bounds causes a cutoff
                if alpha >= beta {
                    return tte.best_value;
                }
            }

            tt_move = tte.best_move;
            static_eval = tte.best_value;
        } else {
            static_eval = evaluate::evaluate(board);
        }

        if board.status() == GameStatus::Won {
            // If the board is in mate, the current side to move has lost
            // MATE_VALUE is unreachable except for mate
            // Subtracting the ply makes the engine look for faster mates
            return -(MATE_VALUE - Value::from(self.ply));
        } else if board.status() == GameStatus::Drawn {
            // If the board is drawn (stalemate or 50-move rule)
            return 0;
        }
        // TODO: Insufficient material draw detection? Other more advanced draws?
        // (e.g. specific king-pawn vs king setups)

        // If we have reached the limit of the current search, evaluate the position using
        // Quiescence search
        if depth == 0 {
            return qsearch(board, alpha, beta, timer, stats);
        }

        // Move Ordering
        // If we put moves more likely to cause cutoffs earlier, we avoid having to search useless moves
        let it = MovesIterator::with_all_moves(
            board,
            tt_move,
            self.killers[usize::from(depth)],
            &self.history,
        );
        let mut best_value = -SCORE_INF;
        let mut best_move = NULL_MOVE;
        // Push the current board hash to the stack for draw detection
        self.push_board_hash(board_hash);

        if !is_pv_node && self.ply > 0 {
            // Null Move Heuristic (NMH) / Null Move Pruning (NMP)
            // This heuristic assumes that we can always improve our position with a legal move.
            // If we forfeit our right to move and still cause a cutoff, then there's no point searching
            // all moves from this position since they'll be better anyway and we just want a cutoff.
            // This is avoided for PV nodes and if the remaining search is shallow anyway. For PV nodes,
            // we want to calculate the line we will play as far as possible to ensure it is good.
            if depth >= 3 {
                let null_move = board.null_move();
                // Null move is not always guaranteed to be legal (King in check)
                if let Some(move_board) = null_move {
                    let null_move_value =
                        -self.search_internal(&move_board, stats, depth - 3, -beta, -beta + 1, timer);
                    if null_move_value >= beta {
                        self.pop_board_hash();
                        return null_move_value;
                    }
                }
            }

            // Reverse Futility Pruning (RFP)
            // This pruning heuristic checks if the current board evaluation (either from TT or a
            // static eval) is enough to cause a cutoff by a significant margin. The margin required
            // scales with depth, discouraging cutoffs at higher depths. The idea is, if the eval is good
            // enough, no decent move will lose hard enough to not cause a cutoff. Thus, we might as well
            // assume a cutoff. Higher depth searches from the same position will fail this check, thus
            // the position will eventually be fully searched.
            if depth <= 5 && board.checkers().is_empty() && static_eval >= (beta + RFP_EVAL_MARGIN * Value::from(depth)) {
                self.pop_board_hash();
                return static_eval;
            }
        }

        for (move_num, (mv, iscapture)) in it.enumerate() {
            let mut move_board = board.clone();
            move_board.play(mv);

            // Principal Value Search (PVS)
            // This heuristic is dependent on having good move ordering. It searches the first move (TT move)
            // fully, assuming that it is likely the best move from this position. In a perfect world, no
            // other move will increase alpha more than this does. So, all subsequent searches are made with
            // a null window centered at alpha which is significantly cheaper. If the returned score is within
            // bounds, it's possible that the current move is better (because it's not a perfect world) so it
            // is searched again with a full window. If the move ordering is good enough, we won't do many
            // researches and overall reduce the time spent searching.
            let cur_value = if move_num == 0 {
                -self.search_internal(&move_board, stats, depth - 1, -beta, -alpha, timer)
            } else {
                let mut reduction = 0;
                // Late Move Reduction (LMR)
                // This heuristic combines with PVS. Since a late move (searched later in the ordering) is
                // unlikely to be good, it shouldn't be searched for the full depth. We only do this depth
                // reduction if the remaining depth is above a threshold, after already having searched a
                // few moves without reduction, and if the move is not a capture, promotion or check.
                // The amount of reduction is based on a formula precomputed in the lmr_table
                if depth >= LMR_MIN_DEPTH
                    && move_num >= (2 + 2 * usize::from(is_pv_node))
                    && !iscapture
                    && mv.promotion.is_none()
                    && move_board.checkers().is_empty()
                {
                    reduction = self.lmr_table.get(depth, move_num);
                    reduction = reduction.clamp(0, depth - 2);
                };

                let new_depth = depth - reduction - 1;
                // Do the null-window search to a reduced depth
                let tmp_value =
                    -self.search_internal(&move_board, stats, new_depth, -alpha - 1, -alpha, timer);
                if alpha < tmp_value && tmp_value < beta {
                    // Re-search happens at the full depth
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
                    // Killer Heuristic
                    // We keep track of non-capture moves that caused a cutoff to rank them higher
                    // in the move ordering, should they be legal again at this depth.
                    self.killers[usize::from(depth)] = Some(mv);
                    // History Heuristic
                    // This argues that board positions don't change very significantly, and if a
                    // move is good now it'll be good later. We maintain a table of values indexed
                    // by which colored piece moved to which square, and use these values to order
                    // non-capture moves.
                    self.history.update(board, mv, depth);
                }

                break;
            }
        }

        self.pop_board_hash();

        // Node type to be stored in the TT
        let node_type = if best_value <= alpha_orig {
            NodeType::UpperBound
        } else if best_value >= beta {
            NodeType::LowerBound
        } else {
            NodeType::Exact
        };

        // Store TT entry
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

        // Save best move at root
        if self.ply == 0 {
            self.best_move = best_move;
        }

        best_value
    }

    // Check if a position is a draw by repetition
    fn is_repetition_draw(&self, halfmove_count: usize, board_hash: u64) -> bool {
        // Can't be a reptition if the halfmove clock (ply since last capture or pawn move) < 4
        if halfmove_count < 4 {
            return false;
        }
        let mut rep_count = 0;
        for &hash in self
            .board_history
            .iter()
            .rev() // Search hashes from recent to old
            .take(halfmove_count) // Only care about the ones after the last capture/pawn move
            .skip(1) // Skip 1 since the first board hash is of the opposite side to move
            .step_by(2)
        // Only look at hashes when it was our turn to move
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

// Quiescence Search (QSearch)
// Instead of directly evaluating a position, evaluate it after there are no possible captures left.
// This helps combat the horizon effect, where we stop searching thinking we are up material not
// realizing that pieces are hanging. To finish faster, this uses alpha-beta pruning too.
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

    // If the evaluation of the current position is enough to cause a cutoff,
    // do it (all captures). Basically similar to NMP.
    let stand_pat = evaluate::evaluate(board);
    if stand_pat >= beta {
        return stand_pat;
    }
    alpha = alpha.max(stand_pat);

    // Only iterate over captures
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

    use crate::search::SearchStats;

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
        let (_, bv) = Searcher::new(10_000_000).search_for_time(
            &mut board,
            &moves,
            &mut stats,
            Duration::from_secs(1),
        );
        assert_eq!(bv, 0);
    }

    #[test]
    fn force_repetition() {
        let mut board =
            Board::from_fen("7k/5pp1/6p1/8/1rn3Q1/qrb5/8/3K4 w - - 0 1", false).unwrap();
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
