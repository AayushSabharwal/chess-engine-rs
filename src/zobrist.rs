use rand::thread_rng;
use cozy_chess::{Board,Color};

pub struct ZobristHasher {
    black_to_move: u64,
    piece_hashes: [u64; 64 * 12],
}

impl ZobristHasher {
    pub fn new() {
        let mut rng = thread_rng();
        let mut ret = Self {
            black_to_move: rng.gen(),
            piece_hashes: [0; 64 * 12],
        };
        for x in ret.piece_hashes.iter_mut() {
            *x = rng.gen();
        }
        ret
    }

    pub fn hash(&self, board: &Board) -> u64 {
        let mut hval = 0;
        if board.side_to_move() == Color::Black {
            hval ^= self.black_to_move;
        }


    }
}
