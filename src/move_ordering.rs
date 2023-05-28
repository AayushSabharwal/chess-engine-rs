use arrayvec::ArrayVec;
use cozy_chess::{Board, Move};

use crate::{evaluate, utils};
use crate::utils::{piece_to_index, NULL_MOVE};

fn move_eval_selection_sort_iter<T,const S: usize>(move_evals: &mut ArrayVec<(Move, T), S>, cur: usize) -> Move where T: Ord {
    let mut i = cur + 1;
    while i < move_evals.len() {
        if move_evals[i].1 > move_evals[cur].1 {
            move_evals.swap(i, cur);
            move_evals.swap(i, cur);
        }

        i += 1;
    }


    move_evals[cur].0
}

pub struct CaptureMoveCollector {
    move_evals: ArrayVec<(Move, (i32, i32)), 100>,
}

impl CaptureMoveCollector {
    pub fn new(board: &Board) -> Self {
        let mut move_evals = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|mut mvs| {
            mvs.to &= enemy;
            let src_eval = evaluate::piece_value(board.piece_on(mvs.from).unwrap());
            for mv in mvs {
                move_evals.push((mv, (evaluate::piece_value(board.piece_on(mv.to).unwrap()), src_eval)));
            }
            false
        });

        Self {
            move_evals,
        }
    }
}

impl IntoIterator for CaptureMoveCollector {
    type Item = Move;

    type IntoIter = CaptureMoveIterator;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            col: self,
            cur: 0,
        }
    }
}

pub struct CaptureMoveIterator {
    col: CaptureMoveCollector,
    cur: usize,
}

impl Iterator for CaptureMoveIterator {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.col.move_evals.len() {
            return None;
        }

        let best_move = move_eval_selection_sort_iter(&mut self.col.move_evals, self.cur);
        self.cur += 1;
        Some(best_move)
    }
}

pub struct ComprehensiveMoveCollector {
    ttmove: Option<Move>,
    ttmove_capture: bool,
    poscaptures: ArrayVec<(Move, (i32, i32)), 100>,
    negcaptures: ArrayVec<(Move, (i32, i32)), 100>,
    noncaptures: ArrayVec<(Move, usize), 218>,
}

impl ComprehensiveMoveCollector {
    pub fn new(board: &Board, killer: Move, history: &Vec<usize>, ttmove: Option<Move>) -> Self {
        let mut poscaptures = ArrayVec::new();
        let mut negcaptures = ArrayVec::new();
        let mut noncaptures = ArrayVec::new();

        let enemy = board.colors(!board.side_to_move());
        board.generate_moves(|moves| {
            let src_piece = board.piece_on(moves.from).unwrap();
            let src_color = board.color_on(moves.from).unwrap();
            let src_eval = evaluate::piece_value(src_piece);
            for mv in moves {
                if enemy.has(mv.to) {
                    let tgt_eval = evaluate::piece_value(board.piece_on(mv.to).unwrap());
                    if tgt_eval >= src_eval {
                        poscaptures.push((mv, (tgt_eval, src_eval)));
                    }
                    else {
                        negcaptures.push((mv, (tgt_eval, src_eval)));
                    }
                    continue;
                }

                if mv == killer {
                    noncaptures.push((mv, usize::MAX));
                    let idx = noncaptures.len() - 1;
                    noncaptures.swap(idx, 0);
                    continue;
                }

                noncaptures.push((mv, history[utils::get_history_index(src_piece, src_color, mv.to)]));
            }
            false
        });

        Self {
            ttmove,
            ttmove_capture: if let Some(mv) = ttmove { board.is_legal(mv) && enemy.has(mv.to) } else {false},
            poscaptures,
            negcaptures,
            noncaptures,
        }
    }
}

impl IntoIterator for ComprehensiveMoveCollector {
    type Item = (bool, Move);

    type IntoIter = ComprehensiveMoveIterator;

    fn into_iter(self) -> Self::IntoIter {
        let done_ttmove = self.ttmove.is_none();
        Self::IntoIter {
            col: self,
            done_ttmove,
            poscapture_cur: 0,
            negcapture_cur: 0,
            noncapture_cur: 0,
        }
    }
}

pub struct ComprehensiveMoveIterator {
    col: ComprehensiveMoveCollector,
    done_ttmove: bool,
    poscapture_cur: usize,
    negcapture_cur: usize,
    noncapture_cur: usize,
}

impl Iterator for ComprehensiveMoveIterator {
    type Item = (bool, Move);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.done_ttmove {
            self.done_ttmove = true;
            Some((self.col.ttmove_capture, self.col.ttmove.unwrap()))
        }
        else if self.poscapture_cur < self.col.poscaptures.len() {
            let best_move = move_eval_selection_sort_iter(&mut self.col.poscaptures, self.poscapture_cur);
            self.poscapture_cur += 1;
            Some((true, best_move))
        }
        else if self.noncapture_cur < self.col.noncaptures.len() {
            let best_move = move_eval_selection_sort_iter(&mut self.col.noncaptures, self.noncapture_cur);
            self.noncapture_cur += 1;
            Some((false, best_move))
        }
        else if self.negcapture_cur < self.col.negcaptures.len() {
            let best_move = move_eval_selection_sort_iter(&mut self.col.negcaptures, self.negcapture_cur);
            self.negcapture_cur += 1;
            Some((true, best_move))
        }
        else {
            None
        }
    }
}


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
