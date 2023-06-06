use crate::types::Depth;

const LMR_BASE: f64 = 0.75;
const LMR_DIVISOR: f64 = 2.25;

#[derive(Debug)]
pub struct LMRTable {
    table: [[Depth; 64]; 64],
}

impl LMRTable {
    #[allow(
        clippy::needless_range_loop,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn new() -> Self {
        let mut table = [[0; 64]; 64];

        for move_num in 0..64 {
            for depth in 0..64 {
                table[move_num][depth] = (LMR_BASE
                    + f64::ln(depth.max(1) as f64) * f64::ln(move_num.max(1) as f64) / LMR_DIVISOR)
                    as Depth;
            }
        }

        Self { table }
    }

    pub fn get(&self, depth: Depth, move_num: usize) -> Depth {
        let midx = move_num.min(63);
        let didx = usize::from(depth.min(63));
        self.table[midx][didx]
    }
}
