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

use crate::{search::SearchStatus, utils::kxr_to_uci_move};
mod evaluate;
mod move_ordering;
mod psqts;
mod search;
mod transposition_table;
mod utils;

#[derive(Debug)]
enum ThreadMessage {
    SearchTask {
        board: Board,
        board_history: Vec<u64>,
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
                panic!("AAA {}", e);
            }
        };

        match task {
            ThreadMessage::SearchTask {
                board,
                board_history,
                time_left,
                time_inc,
            } => {
                let mut status = SearchStatus::new(board_history);
                let (mut bm, _bv) =
                    searcher.search_for_time(&board, &mut status, time_left / 20 + time_inc / 2);

                println!("info nodes {}", status.nodes_visited);

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

fn uci_handler(tx: Sender<ThreadMessage>) {
    let options = UciFormatOptions::default();
    let mut cur_board = Board::startpos();
    let mut board_history = Vec::new();

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
                UciCommand::Position {
                    init_pos,
                    moves: mvs,
                } => {
                    cur_board = Board::from(init_pos);

                    board_history.clear();
                    board_history.push(cur_board.hash());
                    for mut mv in mvs {
                        uci_to_kxr_move(&cur_board, &mut mv);
                        cur_board.play_unchecked(mv);
                        board_history.push(cur_board.hash());

                        if cur_board.halfmove_clock() == 0 {
                            board_history.clear();
                        }
                    }
                    if !board_history.is_empty() {
                        board_history.pop();
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
                        board_history: board_history.clone(),
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
    let mut searcher: Searcher = Searcher::new(100_000_000);
    let mut total_nodes = 0;
    let mut total_time = 0;
    for (i, fen) in include_str!("fen.csv").split('\n').take(50).enumerate() {
        searcher.tt.clear();
        let board = fen.parse::<Board>().unwrap();
        let start = Instant::now();
        let mut status = SearchStatus::new(std::iter::empty());
        let (bm, bv) = searcher.search_fixed_depth(&board, &mut status, 7);
        let duration = start.elapsed();
        total_nodes += status.nodes_visited;
        total_time += duration.as_micros();

        println!(
            "Position [{i:02}]: Move {:} Value {bv:8} | {:10} Nodes in {:6.3}s at {:10.2} KNPS",
            bm,
            status.nodes_visited,
            duration.as_micros() as f64 / 1e6,
            status.nodes_visited as f64 / duration.as_micros() as f64 * 1e3
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
    //
    dbg!(Searcher::new(100_000_000).search_for_time(
        &board,
        &mut SearchStatus::new(std::iter::empty()),
        Duration::from_secs(10)
    ));
}

#[cfg(test)]
mod test {
    use cozy_chess::{Board, GameStatus};
    use std::{fs, time::Duration};

    use crate::search::{SearchStatus, Searcher, MATE_VALUE};

    fn mate_in_i(mate_in: usize, fpath: &str, count: usize) {
        let ply = 2 * mate_in - 1;
        let mut searcher = Searcher::new(100_000_000);
        for fen in fs::read_to_string(fpath).unwrap().split("\n").take(count) {
            let mut board = Board::from_fen(fen, false).unwrap();
            let (mut bm, bv) = searcher.search_for_time(
                &board,
                &mut SearchStatus::new(std::iter::empty()),
                Duration::from_secs(10),
            );
            board.play(bm);
            assert!(bv > MATE_VALUE - 100);
            for _ in 1..ply {
                (bm, _) = searcher.search_for_time(
                    &board,
                    &mut SearchStatus::new(std::iter::empty()),
                    Duration::from_secs(10),
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
