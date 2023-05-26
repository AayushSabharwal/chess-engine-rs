use std::{env, io::stdin, time::Instant};

use crate::search::Searcher;
use cozy_chess::{Board, Move, Square};
use cozy_uci::{
    command::UciCommand,
    remark::{UciIdInfo, UciRemark},
    UciFormatOptions, UciParseErrorKind,
};
use UciParseErrorKind::*;
mod evaluate;
mod move_ordering;
mod search;
mod transposition_table;
mod utils;

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

    let options = UciFormatOptions::default();
    let mut cur_board = Board::startpos();
    let mut searcher: Searcher<10000> = Searcher::new(7);

    loop {
        let mut line = String::new();
        stdin().read_line(&mut line).unwrap();

        match UciCommand::parse_from(&line, &options) {
            Ok(cmd) => match cmd {
                UciCommand::Uci => {
                    println!(
                        "{:?}",
                        UciRemark::Id(UciIdInfo::Name("toy-engine".to_owned()))
                    );

                    println!(
                        "{:?}",
                        UciRemark::Id(UciIdInfo::Author("Aayush Sabharwal".to_owned()))
                    );

                    println!("{:?}", UciRemark::UciOk);
                }
                UciCommand::Debug(_) => todo!(),
                UciCommand::IsReady => println!("{:?}", UciRemark::ReadyOk),
                UciCommand::Position { init_pos, moves } => {
                    match init_pos {
                        cozy_uci::command::UciInitPos::StartPos => cur_board = Board::startpos(),
                        cozy_uci::command::UciInitPos::Board(board) => cur_board = board,
                    }

                    for mv in moves {
                        cur_board.play_unchecked(mv);
                    }
                }
                UciCommand::SetOption { name: _, value: _ } => todo!(),
                UciCommand::UciNewGame => todo!(),
                UciCommand::Stop => todo!(),
                UciCommand::PonderHit => todo!(),
                UciCommand::Quit => break,
                UciCommand::Go(opts) => {
                    let (stats, best_move, best_value) = searcher.search(&cur_board, opts.depth);
                    println!("STATS: {:?}", stats);
                    println!("VALUE: {:?}", best_value);
                    println!(
                        "{:?}",
                        UciRemark::BestMove {
                            mv: best_move.unwrap(),
                            ponder: None
                        }
                    );
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
    let mut searcher: Searcher<10000> = Searcher::new(7);
    let mut total_nodes = 0;
    let mut total_time = 0;
    for (i, fen) in include_str!("fen.csv").split("\n").take(50).enumerate() {
        let board = fen.parse::<Board>().unwrap();
        let start = Instant::now();
        let (stats, bm, bv) = searcher.search(&board, None);
        let duration = start.elapsed();
        total_nodes += stats.nodes_visited;
        total_time += duration.as_micros();

        println!(
            "Position [{i:02}]: Move {:} Value {bv:8} | {:10} Nodes in {:6.3}s at {:10.2} KNPS",
            if bm.is_none() {
                Move {
                    from: Square::A1,
                    to: Square::A1,
                    promotion: None,
                }
            } else {
                bm.unwrap()
            },
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
    let board = "r1br1nk1/ppq1bpp1/4p2p/8/4N2P/P3P3/1PQBBPP1/2R1K2R b K - 0 17"
        .parse::<Board>()
        .unwrap();
    // println!("{board}");
    dbg!(Searcher::<10000>::new(7).search(&board, None));
}
