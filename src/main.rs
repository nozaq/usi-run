#[macro_use]
extern crate clap;
extern crate indicatif;
extern crate toml;
extern crate shogi;

mod config;
mod environment;
mod error;
mod game;
mod reporter;
mod stats;
mod usi;

use std::sync::{Arc, Mutex};
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
use reporter::{Reporter, BoardReporter, UsiReporter, SimpleReporter};

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
            .possible_values(&["board", "command", "simple"])
            .default_value("simple"))
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
        Ok(_) => {} 
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

    let reporter: Arc<Mutex<Reporter + Send + Sync>> = match config.display {
        DisplayMode::Board => Arc::new(Mutex::new(BoardReporter::new(black_engine.score.clone(), white_engine.score.clone()))),
        DisplayMode::Command => Arc::new(Mutex::new(UsiReporter{})),
        DisplayMode::Simple => Arc::new(Mutex::new(SimpleReporter::default())),
    };
    set_command_logger(Color::Black, &mut black_engine, reporter.clone());
    set_command_logger(Color::White, &mut white_engine, reporter.clone());

    black_engine.listen(env.new_sender(), black_rx);
    white_engine.listen(env.new_sender(), white_rx);

    let monitor_handle = start_monitor_thread(&config, monitor_rx, reporter.clone());

    for _ in 0..config.num_games {
        try!(env.start_game(&[&black_tx, &white_tx, &monitor_tx]));
    }

    try!(black_engine.kill());
    try!(white_engine.kill());

    let stats = monitor_handle.join().expect("unexpected error occurred in the monitoring thread");

    reporter.lock().unwrap().on_match_finished(&stats);

    Ok(stats)
}

fn start_monitor_thread(config: &MatchConfig,
                        rx: Receiver<Event>, 
                        reporter: Arc<Mutex<Reporter + Send + Sync>>)
                        -> thread::JoinHandle<MatchStatistics> {
    let num_games = config.num_games;

    thread::spawn(move || {
        let mut results = MatchStatistics::new(num_games);

        while let Some(event) = rx.recv().ok() {
            reporter.lock().unwrap().on_game_event(&event, &results);

            match event {
                Event::GameOver(winner, _) => {
                    results.record_game(winner);
                    if num_games == results.finished_games() {
                        break;
                    }
                }
                _ => {}
            }
        }

        results
    })
}

fn set_command_logger(color: Color, engine: &mut UsiEngine, reporter: Arc<Mutex<Reporter + Send + Sync>>) {
    let read_reporter = reporter.clone();
    let write_reporter = reporter.clone();

    engine.set_read_hook(Some(Box::new(move |output| {
        read_reporter.lock().unwrap().on_receive_command(color, &output);
    })));

    engine.set_write_hook(Some(Box::new(move |command, raw_str| {
        write_reporter.lock().unwrap().on_send_command(color, &command, raw_str);
    })));
}