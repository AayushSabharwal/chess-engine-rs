use cozy_chess::{Color, Move, Piece, Square};

pub const NULL_MOVE: Move = Move {
    from: Square::A1,
    to: Square::A1,
    promotion: None,
};

pub fn piece_to_index(ptype: Piece, pcolor: Color) -> usize {
    ptype as usize + pcolor as usize * 6
}
