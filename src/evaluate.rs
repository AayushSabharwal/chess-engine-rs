use cozy_chess::{Board, Piece};

pub const PIECE_VALUES: [i32; 6] = [100, 250, 300, 500, 900, 10000];

pub fn piece_value(p: Piece) -> i32 {
    PIECE_VALUES[p as usize]
}

pub fn evaluate(board: &Board) -> i32 {
    let side = board.side_to_move();
    let mut eval = 0;

    for ptype in Piece::ALL {
        eval += (board.colored_pieces(side, ptype).len() as i32
            - board.colored_pieces(!side, ptype).len() as i32)
            * piece_value(ptype);
    }
    eval
}
