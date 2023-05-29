use std::{env, io::stdin, time::{Instant, Duration}, thread, sync::mpsc::{self, Sender}};

use cozy_chess::{Board, Move, Square, Color, Rank, File, Piece};
use cozy_uci::{
    command::UciCommand,
    remark::{UciIdInfo, UciRemark},
    UciFormatOptions, UciParseErrorKind,
};
use UciParseErrorKind::*;
use search::Searcher;
mod psqts;
mod evaluate;
mod move_ordering;
mod search;

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

    let mut searcher = Searcher::new(3);

    let options = UciFormatOptions::default();
    loop {
        let task = match rx.recv() {
            Ok(r) => r,
            Err(e) => {panic!("AAA {}", e);}
        };
        println!("{}", task.board);
        let (ss, mut bm, _bv) = searcher.search(&task.board, task.time_left / 40 + task.time_inc / 10);
        task.board.generate_moves(|moves| {
            for mv in moves {
                println!("{:?}", mv);
            }
            false
        });
        println!("info nodes {}", ss.nodes_visited);
        println!("{:?}", bm);

        if task.board.piece_on(bm.from) == Some(Piece::King) && task.board.piece_on(bm.to) == Some(Piece::Rook) {
            bm.to = match (bm.from, bm.to) {
                (Square::E1, Square::H1) => Square::G1,
                (Square::E8, Square::H8) => Square::G8,
                (Square::E1, Square::A1) => Square::C1,
                (Square::E8, Square::A8) => Square::C8,
                _ => bm.to,
            };
        }
        println!("{}", UciRemark::BestMove { mv: bm, ponder: None }.format(&options));
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
                        UciRemark::Id(UciIdInfo::Author("Aayush Sabharwal".to_owned())).format(&options)
                    );

                    println!("{:}", UciRemark::UciOk.format(&options));
                }
                UciCommand::Debug(_) => {},
                UciCommand::IsReady => println!("{:}", UciRemark::ReadyOk.format(&options)),
                UciCommand::Position { init_pos, moves } => {
                    cur_board = Board::from(init_pos);
                    for mut mv in moves {
                        if cur_board.piece_on(mv.from) == Some(Piece::King) && cur_board.piece_on(mv.to) != Some(Piece::Rook) {
                            mv.to = match (mv.from, mv.to) {
                                (Square::E1, Square::G1) => Square::H1,
                                (Square::E8, Square::G8) => Square::H8,
                                (Square::E1, Square::C1) => Square::A1,
                                (Square::E8, Square::C8) => Square::A8,
                                _ => mv.to,
                            };
                        }
                        cur_board.play_unchecked(mv);
                    }
                }
                UciCommand::SetOption { name: _, value: _ } => {},
                UciCommand::UciNewGame => {},
                UciCommand::Stop => {},
                UciCommand::PonderHit => {},
                UciCommand::Quit => {},
                UciCommand::Go(opts) => {
                    tx.send(SearchTask {board: cur_board.clone(), time_left: match cur_board.side_to_move() {
                        Color::White => opts.wtime.unwrap(),
                        Color::Black => opts.btime.unwrap(),
                    }, time_inc: match cur_board.side_to_move() {
                        Color::White => opts.winc.unwrap(),
                        Color::Black => opts.binc.unwrap(),
                    }}).unwrap();
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
    let mut searcher: Searcher = Searcher::new(5);
    let mut total_nodes = 0;
    let mut total_time = 0;
    for (i, fen) in include_str!("fen.csv").split("\n").take(50).enumerate() {
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
    let board = "r1b1kb1r/p2n1pp1/2n1p2p/qpppP3/8/N2P1NP1/PPP1QPBP/R1B2RK1 w kq - 2 10"
        .parse::<Board>()
        .unwrap();
    // println!("{board}");
    for i in 1..=5 {

    dbg!(Searcher::new(i).search(&board, Duration::from_secs(10)));
    }
}
