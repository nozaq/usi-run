use shogi::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::error::Error;
use crate::game::Game;

pub type SharedGame = Arc<RwLock<Game>>;

#[derive(Debug)]
pub enum Action {
    Ready(Color),
    RequestState,
    MakeMove(Color, Move, Instant),
    DeclareWinning(Color),
    Resign(Color),
}

#[derive(Debug, Clone, Copy)]
pub enum GameOverReason {
    Resign,
    IllegalMove,
    OutOfTime,
    MaxPly,
    DeclareWinning,
}

#[derive(Debug, Clone)]
pub enum Event {
    IsReady,
    NewGame(SharedGame),
    NewTurn(SharedGame, Duration),
    NotifyState(SharedGame),
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
            tx: tx,
            rx: rx,
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

    pub fn start_game(&mut self, game: Game, listeners: &[&Sender<Event>]) -> Result<(), Error> {
        let transmit = |event: &Event| -> Result<(), Error> {
            for l in listeners.iter() {
                l.send(event.clone())?;
            }
            Ok(())
        };

        let shared_game = Arc::new(RwLock::new(game));

        transmit(&Event::IsReady)?;
        self.wait_readyok()?;
        transmit(&Event::NewGame(shared_game.clone()))?;

        if let Some(mut game) = shared_game.write().ok() {
            game.turn_start_time = Instant::now();
        }
        transmit(&Event::NewTurn(shared_game.clone(), Duration::from_secs(0)))?;

        while let Some(action) = self.rx.recv().ok() {
            match action {
                Action::RequestState => {
                    transmit(&Event::NotifyState(shared_game.clone()))?;
                }
                Action::MakeMove(c, ref m, ref ts) => {
                    if let Some(mut game) = shared_game.write().ok() {
                        if c != game.pos.side_to_move() {
                            transmit(&Event::GameOver(Some(c), GameOverReason::IllegalMove))?;
                            break;
                        }

                        let elapsed = ts.duration_since(game.turn_start_time);
                        match game.time.consume(c, elapsed) {
                            true => {}
                            false => {
                                transmit(&Event::GameOver(
                                    Some(c.flip()),
                                    GameOverReason::OutOfTime,
                                ))?;
                                break;
                            }
                        }

                        match game.pos.make_move(m) {
                            Ok(_) => {
                                if let Some(max_ply) = self.max_ply {
                                    if game.pos.ply() >= max_ply {
                                        transmit(&Event::GameOver(None, GameOverReason::MaxPly))?;
                                        break;
                                    }
                                }

                                game.turn_start_time = Instant::now();
                                transmit(&Event::NewTurn(shared_game.clone(), elapsed))?;
                            }
                            Err(_) => {
                                transmit(&Event::GameOver(
                                    Some(c.flip()),
                                    GameOverReason::IllegalMove,
                                ))?;
                                break;
                            }
                        }
                    }
                }
                Action::Resign(c) => {
                    if let Some(game) = shared_game.read().ok() {
                        if c != game.pos.side_to_move() {
                            transmit(&Event::GameOver(
                                Some(c.flip()),
                                GameOverReason::IllegalMove,
                            ))?;
                            break;
                        }

                        transmit(&Event::GameOver(
                            Some(c.flip()),
                            GameOverReason::Resign
                        ),)?;
                        break;
                    }
                }
                Action::DeclareWinning(c) => {
                    if let Some(game) = shared_game.read().ok() {
                        if game.pos.try_declare_winning(c) {
                            transmit(&Event::GameOver(
                                Some(c),
                                GameOverReason::DeclareWinning
                            ),)?;
                        } else {
                            transmit(&Event::GameOver(
                                Some(c.flip()),
                                GameOverReason::DeclareWinning,
                            ))?;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn wait_readyok(&self) -> Result<(), Error> {
        let mut state = (false, false);

        // Currently no timeout value is set for waiting "readyok" command.
        while let Some(action) = self.rx.recv().ok() {
            match action {
                Action::Ready(c) => {
                    if c == Color::Black {
                        state.0 = true;
                    } else {
                        state.1 = true
                    }

                    if state.0 && state.1 {
                        return Ok(());
                    }
                }
                _ => { /* ignore other events. */ }
            }
        }

        return Err(Error::EngineNotResponded);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn run_game() {
        let mut env = Environment::new();

        let tx = env.new_sender();
        let (black_tx, black_rx) = channel();
        let (white_tx, white_rx) = channel();

        thread::spawn(move || {
            assert!(match black_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::IsReady) => true,
                _ => false,
            });
            assert!(match white_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::IsReady) => true,
                _ => false,
            });

            assert!(tx.send(Action::Ready(Color::Black)).is_ok());
            assert!(tx.send(Action::Ready(Color::White)).is_ok());

            assert!(match black_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::NewGame(_)) => true,
                _ => false,
            });
            assert!(match white_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::NewGame(_)) => true,
                _ => false,
            });

            assert!(match black_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::NewTurn(_, _)) => true,
                _ => false,
            });
            assert!(match white_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::NewTurn(_, _)) => true,
                _ => false,
            });

            assert!(tx.send(Action::Resign(Color::Black)).is_ok());

            assert!(match black_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::GameOver(Some(Color::White), _)) => true,
                _ => false,
            });
            assert!(match white_rx.recv_timeout(Duration::from_secs(1)).ok() {
                Some(Event::GameOver(Some(Color::White), _)) => true,
                _ => false,
            });
        });

        let tc = TimeControl::Byoyomi {
            black_time: Duration::from_millis(0),
            white_time: Duration::from_millis(0),
            byoyomi: Duration::from_millis(100),
        };

        let game = Game::new(tc.clone());
        let res = env.start_game(game, &[&black_tx, &white_tx]);
        assert!(res.is_ok());
    }
}
