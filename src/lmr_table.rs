use crate::types::Depth;

const LMR_BASE: f64 = 0.77;
const LMR_DIVISOR: f64 = 2.36;

#[derive(Debug)]
pub struct LMRTable {
    table: [[Depth; 257]; 64],
}

impl LMRTable {
    pub fn new() -> Self {
        let mut table = [[0; 257]; 64];

        for move_num in 0..64 {
            for depth in 0..=256 {
                table[move_num][depth] = (LMR_BASE
                    + f64::ln(depth.max(1) as f64) * f64::ln(move_num.max(1) as f64) / LMR_DIVISOR)
                    as Depth;
            }
        }

        Self { table }
    }

    pub fn get(&self, depth: Depth, move_num: usize) -> Depth {
        self.table[move_num.min(63)][usize::from(depth)]
    }
}
