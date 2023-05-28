use cozy_chess::{Board, Piece};

pub const PIECE_VALUES: [i32; 6] = [100, 250, 300, 500, 900, 10000];

pub fn evaluate(board: &Board) -> i32 {
    let cur_side = board.side_to_move();
    let oth_side = !cur_side;
    let mut eval = 0;
    for p in Piece::ALL {
        eval += (board.colored_pieces(cur_side, p).len() as i32
            - board.colored_pieces(oth_side, p).len() as i32)
            * PIECE_VALUES[p as usize];
    }

    eval
}
