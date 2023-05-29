use std::{
    env,
    io::stdin,
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use cozy_chess::{Board, Color};
use cozy_uci::{
    command::UciCommand,
    remark::{UciIdInfo, UciRemark},
    UciFormatOptions, UciParseErrorKind,
};
use search::Searcher;
use utils::uci_to_kxr_move;
use UciParseErrorKind::*;

use crate::utils::kxr_to_uci_move;
mod evaluate;
mod move_ordering;
mod psqts;
mod search;
mod utils;

#[derive(Debug)]
struct SearchTask {
    pub board: Board,
    pub time_left: Duration,
    pub time_inc: Duration,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        if args[1] == "bench" {
            run_benchmark();
        }
        if args[1] == "hyperfine" {
            hyperfine();
        }
        return;
    }

    let (tx, rx) = mpsc::channel::<SearchTask>();

    let _handler = thread::spawn(move || {
        uci_handler(tx);
    });

    let searcher = Searcher::new(3);

    let options = UciFormatOptions::default();
    loop {
        let task = match rx.recv() {
            Ok(r) => r,
            Err(e) => {
                panic!("AAA {}", e);
            }
        };

        let (ss, mut bm, _bv) =
            searcher.search(&task.board, task.time_left / 40 + task.time_inc / 10);

        println!("info nodes {}", ss.nodes_visited);

        kxr_to_uci_move(&task.board, &mut bm);
        println!(
            "{}",
            UciRemark::BestMove {
                mv: bm,
                ponder: None
            }
            .format(&options)
        );
    }
}

fn uci_handler(tx: Sender<SearchTask>) {
    let options = UciFormatOptions::default();
    let mut cur_board = Board::startpos();

    loop {
        let mut line = String::new();
        stdin().read_line(&mut line).unwrap();

        match UciCommand::parse_from(&line, &options) {
            Ok(cmd) => match cmd {
                UciCommand::Uci => {
                    println!(
                        "{:}",
                        UciRemark::Id(UciIdInfo::Name("toy-engine".to_owned())).format(&options)
                    );

                    println!(
                        "{:}",
                        UciRemark::Id(UciIdInfo::Author("Aayush Sabharwal".to_owned()))
                            .format(&options)
                    );

                    println!("{:}", UciRemark::UciOk.format(&options));
                }
                UciCommand::Debug(_) => {}
                UciCommand::IsReady => println!("{:}", UciRemark::ReadyOk.format(&options)),
                UciCommand::Position { init_pos, moves } => {
                    cur_board = Board::from(init_pos);
                    for mut mv in moves {
                        uci_to_kxr_move(&cur_board, &mut mv);
                        cur_board.play_unchecked(mv);
                    }
                }
                UciCommand::SetOption { name: _, value: _ } => {}
                UciCommand::UciNewGame => {}
                UciCommand::Stop => {}
                UciCommand::PonderHit => {}
                UciCommand::Quit => {}
                UciCommand::Go(opts) => {
                    tx.send(SearchTask {
                        board: cur_board.clone(),
                        time_left: match cur_board.side_to_move() {
                            Color::White => opts.wtime.unwrap(),
                            Color::Black => opts.btime.unwrap(),
                        },
                        time_inc: match cur_board.side_to_move() {
                            Color::White => opts.winc.unwrap(),
                            Color::Black => opts.binc.unwrap(),
                        },
                    })
                    .unwrap();
                }
            },
            Err(err) => {
                if !matches!(err.kind, UnknownMessageKind(_)) {
                    println!("{}", err);
                    continue;
                }
            }
        }
    }
}

fn run_benchmark() {
    let searcher: Searcher = Searcher::new(5);
    let mut total_nodes = 0;
    let mut total_time = 0;
    for (i, fen) in include_str!("fen.csv").split('\n').take(50).enumerate() {
        let board = fen.parse::<Board>().unwrap();
        let start = Instant::now();
        let (stats, bm, bv) = searcher.search(&board, Duration::from_secs(10));
        let duration = start.elapsed();
        total_nodes += stats.nodes_visited;
        total_time += duration.as_micros();

        println!(
            "Position [{i:02}]: Move {:} Value {bv:8} | {:10} Nodes in {:6.3}s at {:10.2} KNPS",
            bm,
            stats.nodes_visited,
            duration.as_micros() as f64 / 1e6,
            stats.nodes_visited as f64 / duration.as_micros() as f64 * 1e3
        );
    }

    println!(
        "Total: {:12} Nodes in {:6.3}s at {:10.2} NPS",
        total_nodes,
        total_time as f64 / 1e6,
        total_nodes as f64 / total_time as f64 * 1e3
    );
}

fn hyperfine() {
    // let board = "r1br1nk1/ppq1bpp1/4p2p/8/4N2P/P3P3/1PQBBPP1/2R1K2R b K - 0 17"
    let board = "r5rk/pp1np1bn/2pp2q1/3P1bN1/2P1N2Q/1P6/PB2PPBP/3R1RK1 w - - 0 1"
        .parse::<Board>()
        .unwrap();
    // println!("{board}");
    for i in 1..=5 {
        dbg!(Searcher::new(i).search(&board, Duration::from_secs(10)));
    }
}

#[cfg(test)]
mod test {
    use std::{fs, time::Duration};

    use cozy_chess::{Board, GameStatus};

    use crate::search::{Searcher, MATE_VALUE};

    fn mate_in_i(mate_in: usize, fpath: &str, count: usize) {
        let ply = 2 * mate_in - 1;
        let searcher = Searcher::new(ply);
        for fen in fs::read_to_string(fpath).unwrap().split("\n").take(count) {
            let mut board = Board::from_fen(fen, false).unwrap();
            let (_, mut bm, bv) = searcher.search(&board, Duration::from_secs(10));
            board.play(bm);
            assert!(bv > MATE_VALUE - 100);
            for _ in 1..ply {
                (_, bm, _) = searcher.search(&board, Duration::from_secs(10));
                board.play(bm);
            }
            assert_eq!(board.status(), GameStatus::Won);
        }
    }

    #[test]
    fn test_mate_in_one() {
        mate_in_i(1, "test_data/m1.txt", 64);
    }

    #[test]
    fn test_mate_in_two() {
        mate_in_i(2, "test_data/m2.txt", 100);
    }

}
