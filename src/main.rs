#[macro_use]
extern crate clap;
extern crate toml;
extern crate shogi;

mod config;
mod environment;
mod error;
mod game;
mod stats;
mod usi;

use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use clap::{App, Arg};
use shogi::Color;
use shogi::bitboard::Factory;

use config::*;
use environment::*;
use error::*;
use stats::*;
use usi::*;

fn main() {
    let matches = App::new("usirun")
        .version(crate_version!())
        .about("A command line utility for running games between USI compliant Shogi engines.")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("TOML")
            .help("Loads a configuration file for setting up match rules")
            .required(true)
            .takes_value(true))
        .arg(Arg::with_name("display")
            .short("d")
            .long("display")
            .value_name("MODE")
            .help("Displays ")
            .takes_value(true)
            .possible_values(&["board", "command", "simple"]))
        .get_matches();

    let mut match_config = MatchConfig::default();
    if let Some(config_path) = matches.value_of("config") {
        match_config.load(config_path)
            .expect(&format!("failed to open the config file at {}", config_path));
    }

    if let Some(display) = matches.value_of("display") {
        match_config.display = match display {
            "board" => DisplayMode::Board,
            "command" => DisplayMode::Command,
            _ => DisplayMode::Simple,
        }
    }

    Factory::init();

    match run_match(&match_config) {
        Ok(ref stats) => {
            println!("");
            println!("Match statistics");
            println!("Black: {} ({:.2}%)",
                     stats.black_wins(),
                     stats.black_win_rate() * 100.0);
            println!("White: {} ({:.2}%)",
                     stats.white_wins(),
                     stats.white_win_rate() * 100.0);
            println!("Draw : {} ({:.2}%)",
                     stats.draw_games(),
                     stats.draw_game_rate() * 100.0);

        } 
        Err(e) => {
            println!("an error occurred during the match: {}", e);
        }
    }
}

fn run_match(config: &MatchConfig) -> Result<MatchStatistics, Error> {
    let mut env = Environment::new(&config.time.to_time_control())
        .initial_sfen(&config.initial_pos)
        .max_ply(config.max_ply);

    let (black_tx, black_rx) = channel();
    let (white_tx, white_rx) = channel();
    let (monitor_tx, monitor_rx) = channel();

    let mut black_engine = try!(UsiEngine::launch(Color::Black, &config.black_engine));
    let mut white_engine = try!(UsiEngine::launch(Color::White, &config.white_engine));

    if config.display == DisplayMode::Command {
        set_command_logger(Color::Black, &mut black_engine);
        set_command_logger(Color::White, &mut white_engine);
    }

    black_engine.listen(env.new_sender(), black_rx);
    white_engine.listen(env.new_sender(), white_rx);

    let monitor_handle = start_monitor_thread(&config, &black_engine, &white_engine, monitor_rx);

    for _ in 0..config.num_games {
        try!(env.start_game(&[&black_tx, &white_tx, &monitor_tx]));

        if config.display == DisplayMode::Simple {
            print!(".");
            try!(std::io::stdout().flush());
        }
    }

    try!(black_engine.kill());
    try!(white_engine.kill());

    let stats = monitor_handle.join().expect("unexpected error occurred in the monitoring thread");

    Ok(stats)
}

fn start_monitor_thread(config: &MatchConfig,
                        black_engine: &UsiEngine,
                        white_engine: &UsiEngine,
                        rx: Receiver<Event>)
                        -> thread::JoinHandle<MatchStatistics> {
    let num_games = config.num_games;
    let display = config.display;
    let black_score = black_engine.score.clone();
    let white_score = white_engine.score.clone();

    thread::spawn(move || {
        let mut results = MatchStatistics::default();

        while let Some(event) = rx.recv().ok() {
            match event {
                Event::NewTurn(game) => {
                    if display == DisplayMode::Board {
                        if let Some(game) = game.read().ok() {
                            println!("{}", game.pos);
                            println!("Time: (Black) {}s, (White) {}s",
                                     game.time.black_time().as_secs(),
                                     game.time.white_time().as_secs());
                            println!("Score (Black) {}, (White) {}",
                                     black_score.load(Ordering::Relaxed),
                                     white_score.load(Ordering::Relaxed));
                            println!("");
                        }
                    }
                }
                Event::GameOver(winner, reason) => {
                    if display == DisplayMode::Board {
                        match winner {
                            Some(c) => {
                                let name = if c == Color::Black { "Black" } else { "White" };
                                println!("Game #{}: {} wins({:?})",
                                         results.total_games() + 1,
                                         name,
                                         reason);
                            }
                            None => {
                                println!("Game #{}: Draw({:?})", results.total_games() + 1, reason);
                            }
                        }
                    }

                    results.record_game(winner);
                    if num_games == results.total_games() {
                        break;
                    }
                }
                _ => {}
            }
        }

        results
    })
}

fn set_command_logger(color: Color, engine: &mut UsiEngine) {
    let prefix = if color == Color::Black { "B" } else { "W" };

    engine.set_read_hook(Some(Box::new(move |output| {
        print!("{}> {}", prefix, output.raw_str());
    })));

    engine.set_write_hook(Some(Box::new(move |_, raw_str| {
        print!("{}< {}", prefix, raw_str);
    })));
}