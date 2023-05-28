use cozy_chess::{Color, Move, Piece, Square};

pub const NULL_MOVE: Move = Move {
    from: Square::A1,
    to: Square::A1,
    promotion: None,
};

#[inline]
pub fn piece_to_index(ptype: Piece, pcolor: Color) -> usize {
    ptype as usize + pcolor as usize * 6
}

pub fn get_history_index(fromptype: Piece, frompcolor: Color, tosq: Square) -> usize {
    piece_to_index(fromptype, frompcolor) * 64 + tosq as usize
}
