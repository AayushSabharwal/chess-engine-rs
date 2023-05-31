use cozy_chess::{Board, Color, Square};

use crate::psqts::{EG_TABLE, MG_TABLE, EG_VALUE, MG_VALUE, GAME_PHASE_INC};

pub const PIECE_VALUES: [i32; 6] = [100, 250, 300, 500, 900, 10000];

pub fn evaluate(board: &Board) -> i32 {
    let cur_side = board.side_to_move();
    let oth_side = !cur_side;
    let mut eg = [0; 2];
    let mut mg = [0; 2];
    let mut game_phase = 0;

    let empty = !board.occupied();
    for i in Square::ALL {
        if empty.has(i) {
            continue;
        }
        let ptype = board.piece_on(i).unwrap();
        let pcol = board.color_on(i).unwrap();

        let mut tb_idx = i as usize;
        if pcol == Color::White {
            tb_idx ^= 0b111_000;
        }
        tb_idx += ptype as usize * 64;

        eg[pcol as usize] += EG_VALUE[ptype as usize] + EG_TABLE[tb_idx];
        mg[pcol as usize] += MG_VALUE[ptype as usize] + MG_TABLE[tb_idx];
        game_phase += GAME_PHASE_INC[ptype as usize];
    }

    let mg_eval = mg[cur_side as usize] - mg[oth_side as usize];
    let eg_eval = eg[cur_side as usize] - eg[oth_side as usize];
    let mg_phase = game_phase.min(24);
    let eg_phase = 24 - mg_phase;
    (mg_eval * mg_phase + eg_eval * eg_phase) / 24
}
