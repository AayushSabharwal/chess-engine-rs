use cozy_chess::{Board, Move, Piece, Square};

pub const NULL_MOVE: Move = Move {
    from: Square::A1,
    to: Square::A1,
    promotion: None,
};

pub fn uci_to_kxr_move(board: &Board, mv: &mut Move) {
    if board.piece_on(mv.from) == Some(Piece::King) && board.piece_on(mv.to) != Some(Piece::Rook) {
        mv.to = match (mv.from, mv.to) {
            (Square::E1, Square::G1) => Square::H1,
            (Square::E8, Square::G8) => Square::H8,
            (Square::E1, Square::C1) => Square::A1,
            (Square::E8, Square::C8) => Square::A8,
            _ => mv.to,
        };
    }
}

pub fn kxr_to_uci_move(board: &Board, mv: &mut Move) {
    if board.piece_on(mv.from) == Some(Piece::King) && board.piece_on(mv.to) == Some(Piece::Rook) {
        mv.to = match (mv.from, mv.to) {
            (Square::E1, Square::H1) => Square::G1,
            (Square::E8, Square::H8) => Square::G8,
            (Square::E1, Square::A1) => Square::C1,
            (Square::E8, Square::A8) => Square::C8,
            _ => mv.to,
        };
    }
}
