use shogi::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::MatchConfig;
use crate::engine::{UsiEngine, WriteHookFn};
use crate::error::Error;
use crate::game::{Game, GameOverReason, GameResult};
use crate::reporter::Reporter;
use crate::stats::MatchStatistics;

const DEFAULT_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - \
                                    1";

#[derive(Debug)]
pub enum Action {
    Ready(Color),
    RequestState,
    MakeMove(Color, Move, Instant),
    DeclareWinning(Color),
    Resign(Color),
}

#[derive(Debug)]
pub enum Event<'a> {
    IsReady,
    NewGame(&'a mut Game),
    NewTurn(&'a mut Game, Duration),
    NotifyState(&'a mut Game),
    GameOver(Option<Color>, GameOverReason),
}

pub struct Environment {
    tx: Sender<Action>,
    rx: Receiver<Action>,
    max_ply: Option<u16>,
}

impl Environment {
    pub fn new() -> Environment {
        let (tx, rx) = channel();

        Environment {
            tx,
            rx,
            max_ply: None,
        }
    }

    pub fn max_ply(mut self, ply: Option<u16>) -> Environment {
        self.max_ply = ply;
        self
    }

    pub fn new_sender(&self) -> Sender<Action> {
        self.tx.clone()
    }

    pub fn start_game(
        &mut self,
        config: &MatchConfig,
        stats: &MatchStatistics,
        black_engine: &mut UsiEngine,
        white_engine: &mut UsiEngine,
        reporter: Arc<Mutex<dyn Reporter + Send>>,
    ) -> Result<GameResult, Error> {
        let mut game = Game::new(config.time.to_time_control());
        game.black_player = black_engine.name.to_string();
        game.white_player = white_engine.name.to_string();
        game.pos
            .set_sfen(config.initial_pos.as_ref().map_or(DEFAULT_SFEN, |v| v))?;

        let mut black_write_hook = Some(create_write_hook(Color::Black, reporter.clone()));
        let mut white_write_hook = Some(create_write_hook(Color::Black, reporter.clone()));

        let mut transmit = |event: &Event| -> Result<(), Error> {
            black_engine.notify_event(event, &mut black_write_hook)?;
            white_engine.notify_event(event, &mut white_write_hook)?;

            if let Ok(mut reporter) = reporter.lock() {
                reporter.on_game_event(event, stats);
            }
            Ok(())
        };

        transmit(&Event::IsReady)?;
        self.wait_readyok()?;
        transmit(&Event::NewGame(&mut game))?;
        game.turn_start_time = Instant::now();

        transmit(&Event::NewTurn(&mut game, Duration::from_secs(0)))?;

        let mut result: Option<GameResult> = None;
        while let Ok(action) = self.rx.recv() {
            match action {
                Action::RequestState => {
                    transmit(&Event::NotifyState(&mut game))?;
                }
                Action::MakeMove(c, ref m, ref ts) => {
                    if c != game.pos.side_to_move() {
                        result = Some(GameResult::new(Some(c), GameOverReason::IllegalMove));
                        break;
                    }

                    let elapsed = ts.duration_since(game.turn_start_time);
                    if !game.time.consume(c, elapsed) {
                        result = Some(GameResult::new(Some(c.flip()), GameOverReason::OutOfTime));
                        break;
                    }

                    match game.pos.make_move(*m) {
                        Ok(_) => {
                            if let Some(max_ply) = self.max_ply {
                                if game.pos.ply() >= max_ply {
                                    result = Some(GameResult::new(None, GameOverReason::MaxPly));
                                    break;
                                }
                            }

                            game.turn_start_time = Instant::now();
                            transmit(&Event::NewTurn(&mut game, elapsed))?;
                        }
                        Err(_) => {
                            result =
                                Some(GameResult::new(Some(c.flip()), GameOverReason::IllegalMove));
                            break;
                        }
                    }
                }
                Action::Resign(c) => {
                    if c != game.pos.side_to_move() {
                        result = Some(GameResult::new(Some(c.flip()), GameOverReason::IllegalMove));
                        break;
                    }

                    result = Some(GameResult::new(Some(c.flip()), GameOverReason::Resign));
                    break;
                }
                Action::DeclareWinning(c) => {
                    if game.pos.try_declare_winning(c) {
                        result = Some(GameResult::new(Some(c), GameOverReason::DeclareWinning));
                    } else {
                        result = Some(GameResult::new(
                            Some(c.flip()),
                            GameOverReason::DeclareWinning,
                        ));
                    }
                }
                _ => {}
            }
        }

        if let Some(result) = &result {
            transmit(&Event::GameOver(result.winner, result.reason))?;
        }

        Ok(result.unwrap())
    }

    fn wait_readyok(&self) -> Result<(), Error> {
        let mut state = (false, false);

        // Currently no timeout value is set for waiting "readyok" command.
        while let Ok(action) = self.rx.recv() {
            if let Action::Ready(c) = action {
                if c == Color::Black {
                    state.0 = true;
                } else {
                    state.1 = true
                }

                if state.0 && state.1 {
                    return Ok(());
                }
            }
        }

        Err(Error::EngineNotResponded)
    }
}

fn create_write_hook(color: Color, reporter: Arc<Mutex<dyn Reporter + Send>>) -> WriteHookFn {
    let write_reporter = reporter.clone();

    Box::new(move |command, raw_str| {
        write_reporter
            .lock()
            .unwrap()
            .on_send_command(color, command, raw_str);
    })
}
