#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::similar_names,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]
use std::{
    env,
    io::stdin,
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use cozy_chess::{Board, Color, Move};
use cozy_uci::{
    command::UciCommand,
    remark::{UciIdInfo, UciRemark},
    UciFormatOptions, UciParseErrorKind,
};
use search::Searcher;
use UciParseErrorKind::UnknownMessageKind;

use crate::{search::SearchStats, utils::kxr_to_uci_move};
mod evaluate;
mod history;
mod move_ordering;
mod psqts;
mod search;
mod transposition_table;
mod types;
mod utils;

#[derive(Debug)]
enum ThreadMessage {
    SearchTask {
        board: Board,
        moves: Vec<Move>,
        time_left: Duration,
        time_inc: Duration,
    },
    NewGame,
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

    let (tx, rx) = mpsc::channel::<ThreadMessage>();

    let _handler = thread::spawn(move || {
        uci_handler(tx);
    });

    let mut searcher = Searcher::new(100_000_000);

    let options = UciFormatOptions::default();
    loop {
        let task = match rx.recv() {
            Ok(r) => r,
            Err(e) => {
                panic!("AAA {e}");
            }
        };

        match task {
            ThreadMessage::SearchTask {
                mut board,
                moves,
                time_left,
                time_inc,
            } => {
                let mut stats = SearchStats::default();
                let (mut bm, _bv) = searcher.search_for_time(
                    &mut board,
                    &moves,
                    &mut stats,
                    time_left / 20 + time_inc / 2,
                );

                println!("info nodes {}", stats.nodes_visited);
                println!("info depth {}", stats.depth);
                kxr_to_uci_move(&board, &mut bm);
                println!(
                    "{}",
                    UciRemark::BestMove {
                        mv: bm,
                        ponder: None
                    }
                    .format(&options)
                );
            }
            ThreadMessage::NewGame => {
                searcher.new_game();
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn uci_handler(tx: Sender<ThreadMessage>) {
    let options = UciFormatOptions::default();
    let mut cur_board = Board::startpos();
    let mut moves = Vec::new();
    moves.reserve(512);

    loop {
        let mut line = String::new();
        stdin().read_line(&mut line).unwrap();

        #[allow(clippy::match_same_arms)]
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
                UciCommand::Position {
                    init_pos,
                    moves: mvs,
                } => {
                    cur_board = Board::from(init_pos);

                    moves.clear();
                    for mv in mvs {
                        moves.push(mv);
                    }
                }
                UciCommand::SetOption { name: _, value: _ } => {}
                UciCommand::UciNewGame => {
                    tx.send(ThreadMessage::NewGame).unwrap();
                }
                UciCommand::Stop => {}
                UciCommand::PonderHit => {}
                UciCommand::Quit => {}
                UciCommand::Go(opts) => {
                    tx.send(ThreadMessage::SearchTask {
                        board: cur_board.clone(),
                        moves: moves.clone(),
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
                    println!("{err}");
                    continue;
                }
            }
        }
    }
}

fn run_benchmark() {
    let mut searcher: Searcher = Searcher::new(100_000_000);
    let mut total_nodes = 0;
    let mut total_time = 0.0;
    let moves = Vec::new();
    for (i, fen) in include_str!("fen.csv").split('\n').take(50).enumerate() {
        searcher.tt.clear();
        let mut board = fen.parse::<Board>().unwrap();
        let start = Instant::now();
        let mut stats = SearchStats::default();
        let (bm, bv) = searcher.search_fixed_depth(&mut board, &moves, &mut stats, 7);
        let duration = start.elapsed();
        total_nodes += stats.nodes_visited;
        total_time += duration.as_secs_f64();

        println!(
            "Position [{i:02}]: Move {:} Value {bv:8} | {:10} Nodes in {:6.3}s at {:10.2} KNPS",
            bm,
            stats.nodes_visited,
            duration.as_secs_f64(),
            f64::from(stats.nodes_visited) / duration.as_secs_f64() / 1e3,
        );
    }

    println!(
        "Total: {:12} Nodes in {:6.3}s at {:10.2} NPS",
        total_nodes,
        total_time,
        f64::from(total_nodes) / total_time / 1e3
    );
}

fn hyperfine() {
    // let board = "r1br1nk1/ppq1bpp1/4p2p/8/4N2P/P3P3/1PQBBPP1/2R1K2R b K - 0 17"
    let mut board = "r5rk/pp1np1bn/2pp2q1/3P1bN1/2P1N2Q/1P6/PB2PPBP/3R1RK1 w - - 0 1"
        .parse::<Board>()
        .unwrap();
    // println!("{board}");
    //
    dbg!(Searcher::new(100_000_000).search_for_time(
        &mut board,
        &Vec::new(),
        &mut SearchStats::default(),
        Duration::from_secs(10)
    ));
}

#[cfg(test)]
mod test {
    use cozy_chess::{Board, GameStatus};
    use std::{fs, time::Duration};

    use crate::search::{SearchStats, Searcher, MATE_VALUE};

    fn mate_in_i(mate_in: usize, fpath: &str, count: usize) {
        let ply = 2 * mate_in - 1;
        let mut searcher = Searcher::new(100_000_000);
        for fen in fs::read_to_string(fpath).unwrap().split("\n").take(count) {
            let mut board = Board::from_fen(fen, false).unwrap();
            let (mut bm, bv) = searcher.search_for_time(
                &mut board,
                &Vec::new(),
                &mut SearchStats::default(),
                Duration::from_millis(100),
            );
            board.play(bm);

            assert!(bv > MATE_VALUE - 100);
            for _ in 1..ply {
                (bm, _) = searcher.search_for_time(
                    &mut board,
                    &Vec::new(),
                    &mut SearchStats::default(),
                    Duration::from_millis(100),
                );
                board.play(bm);
            }
            assert_eq!(board.status(), GameStatus::Won);
        }
    }

    #[test]
    fn mate_in_one() {
        mate_in_i(1, "test_data/m1.txt", 64);
    }

    #[test]
    fn mate_in_two() {
        mate_in_i(2, "test_data/m2.txt", 100);
    }
}
