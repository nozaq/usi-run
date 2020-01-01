mod config;
mod environment;
mod error;
mod game;
mod player;
mod reporter;
mod stats;

use clap::{crate_version, App, Arg};
use shogi::bitboard::Factory;
use shogi::Color;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::error::Error;
use config::*;
use environment::*;
use game::*;
use player::*;
use reporter::{BoardReporter, CsaReporter, Reporter, SimpleReporter, UsiReporter};
use stats::*;

const DEFAULT_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - \
                                    1";

fn main() {
    let matches = App::new("usirun")
        .version(crate_version!())
        .about("A command line utility for running games between USI compliant Shogi engines.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("TOML")
                .help("Loads a configuration file for setting up match rules")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("display")
                .short("d")
                .long("display")
                .value_name("MODE")
                .help("Displays ")
                .takes_value(true)
                .possible_values(&["board", "csa", "command", "simple"])
                .default_value("simple"),
        )
        .get_matches();

    let mut match_config = MatchConfig::default();
    if let Some(config_path) = matches.value_of("config") {
        match_config
            .load(config_path)
            .unwrap_or_else(|_| panic!("failed to open the config file at {}", config_path));
    }

    if let Some(display) = matches.value_of("display") {
        match_config.display = match display {
            "board" => DisplayMode::Board,
            "command" => DisplayMode::Command,
            "csa" => DisplayMode::Csa,
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
    let mut env = Environment::new().max_ply(config.max_ply);

    let (black_tx, black_rx) = channel();
    let (white_tx, white_rx) = channel();
    let (monitor_tx, monitor_rx) = channel();

    let mut black_engine = UsiEngine::launch(Color::Black, &config.black_engine)?;
    let mut white_engine = UsiEngine::launch(Color::White, &config.white_engine)?;

    let reporter: Arc<Mutex<dyn Reporter + Send + Sync>> = match config.display {
        DisplayMode::Board => Arc::new(Mutex::new(BoardReporter::new(
            black_engine.score.clone(),
            white_engine.score.clone(),
        ))),
        DisplayMode::Command => Arc::new(Mutex::new(UsiReporter::default())),
        DisplayMode::Csa => Arc::new(Mutex::new(CsaReporter::default())),
        DisplayMode::Simple => Arc::new(Mutex::new(SimpleReporter::default())),
    };
    set_command_logger(Color::Black, &mut black_engine, reporter.clone());
    set_command_logger(Color::White, &mut white_engine, reporter.clone());

    black_engine.listen(env.new_sender(), black_rx);
    white_engine.listen(env.new_sender(), white_rx);

    let monitor_handle = start_monitor_thread(&config, monitor_rx, reporter.clone());

    for _ in 0..config.num_games {
        let mut game = Game::new(config.time.to_time_control());
        game.black_player = black_engine.name.to_string();
        game.white_player = white_engine.name.to_string();
        game.pos
            .set_sfen(config.initial_pos.as_ref().map_or(DEFAULT_SFEN, |v| v))?;

        env.start_game(game, &[&black_tx, &white_tx, &monitor_tx])?;
    }

    black_engine.kill()?;
    white_engine.kill()?;

    let stats = monitor_handle
        .join()
        .expect("unexpected error occurred in the monitoring thread");

    reporter.lock().unwrap().on_match_finished(&stats);

    Ok(stats)
}

fn start_monitor_thread(
    config: &MatchConfig,
    rx: Receiver<Event>,
    reporter: Arc<Mutex<dyn Reporter + Send + Sync>>,
) -> thread::JoinHandle<MatchStatistics> {
    let num_games = config.num_games;

    thread::spawn(move || {
        let mut results = MatchStatistics::new(num_games);

        while let Some(event) = rx.recv().ok() {
            reporter.lock().unwrap().on_game_event(&event, &results);

            if let Event::GameOver(winner, _) = event {
                results.record_game(winner);
                if num_games == results.finished_games() {
                    break;
                }
            }
        }

        results
    })
}

fn set_command_logger(
    color: Color,
    engine: &mut UsiEngine,
    reporter: Arc<Mutex<dyn Reporter + Send + Sync>>,
) {
    let read_reporter = reporter.clone();
    let write_reporter = reporter.clone();

    engine.set_read_hook(Some(Box::new(move |output| {
        read_reporter
            .lock()
            .unwrap()
            .on_receive_command(color, &output);
    })));

    engine.set_write_hook(Some(Box::new(move |command, raw_str| {
        write_reporter
            .lock()
            .unwrap()
            .on_send_command(color, &command, raw_str);
    })));
}
