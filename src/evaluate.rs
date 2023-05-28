use cozy_chess::{Board, Square, Color};
use crate::psqts::*;

pub const PIECE_VALUES: [i32; 6] = [100, 250, 300, 500, 900, 10000];


pub fn evaluate(board: &Board) -> i32 {
    let cur_side = board.side_to_move();
    let mut eval = 0;

    let empty = !board.occupied();
    for i in Square::ALL {
        if empty.has(i) {
            continue;
        }
        let ptype = board.piece_on(i).unwrap();
        let pcol = board.color_on(i).unwrap();
        eval += if pcol == cur_side { PIECE_VALUES[ptype as usize] } else { -PIECE_VALUES[ptype as usize] };
        if pcol == Color::Black {
            eval += EG_TABLE[(ptype as usize) * 64 + i as usize];
        }
        else {
            eval += EG_TABLE[(ptype as usize) * 64 + ((i as usize) ^ 0b111_000)];
        }
    }

    eval
}
