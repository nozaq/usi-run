mod config;
mod engine;
mod environment;
mod error;
mod game;
mod reporter;
mod stats;

use clap::{crate_version, Arg, Command};
use shogi::bitboard::Factory;
use shogi::Color;
use std::sync::{Arc, Mutex, RwLock};

use crate::error::Error;
use config::*;
use engine::*;
use environment::*;
use reporter::{BoardReporter, CsaReporter, Reporter, SimpleReporter, UsiReporter};
use stats::*;

fn main() {
    let matches = Command::new("usirun")
        .version(crate_version!())
        .about("A command line utility for running games between USI compliant Shogi engines.")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("TOML")
                .help("Loads a configuration file for setting up match rules")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("display")
                .short('d')
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
    let mut stats = MatchStatistics::new(config.num_games);

    let black_state = Arc::new(RwLock::new(ThinkState::default()));
    let white_state = Arc::new(RwLock::new(ThinkState::default()));

    let reporter: Arc<Mutex<dyn Reporter + Send>> = match config.display {
        DisplayMode::Board => Arc::new(Mutex::new(BoardReporter::new(
            black_state.clone(),
            white_state.clone(),
        ))),
        DisplayMode::Command => Arc::new(Mutex::new(UsiReporter::default())),
        DisplayMode::Csa => Arc::new(Mutex::new(CsaReporter::default())),
        DisplayMode::Simple => Arc::new(Mutex::new(SimpleReporter::default())),
    };
    let mut black_engine = UsiEngine::new(
        Color::Black,
        &config.black_engine,
        env.new_sender(),
        Some(create_read_hook(Color::Black, reporter.clone())),
        black_state,
    )?;
    let mut white_engine = UsiEngine::new(
        Color::White,
        &config.white_engine,
        env.new_sender(),
        Some(create_read_hook(Color::Black, reporter.clone())),
        white_state,
    )?;

    for _ in 0..config.num_games {
        let result = env.start_game(
            config,
            &stats,
            &mut black_engine,
            &mut white_engine,
            reporter.clone(),
        )?;
        stats.record_game(result.winner);
    }

    reporter.lock().unwrap().on_match_finished(&stats);

    Ok(stats)
}

fn create_read_hook(color: Color, reporter: Arc<Mutex<dyn Reporter + Send>>) -> ReadHookFn {
    let read_reporter = reporter.clone();
    Box::new(move |output| -> Result<(), Error> {
        read_reporter
            .lock()
            .unwrap()
            .on_receive_command(color, output);

        Ok(())
    })
}
