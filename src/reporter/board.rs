use console::Term;
use std::sync::{Arc, RwLock};

use crate::environment::Event;
use crate::stats::MatchStatistics;
use shogi::Color;

use super::Reporter;
use crate::engine::ThinkState;
use crate::game::{Game, GameOverReason};

pub struct BoardReporter {
    dirty: bool,
    black_state: Arc<RwLock<ThinkState>>,
    white_state: Arc<RwLock<ThinkState>>,
}

impl BoardReporter {
    pub fn new(
        black_state: Arc<RwLock<ThinkState>>,
        white_state: Arc<RwLock<ThinkState>>,
    ) -> BoardReporter {
        BoardReporter {
            dirty: false,
            black_state,
            white_state,
        }
    }

    fn on_new_turn(&mut self, game: &Game, stats: &MatchStatistics) -> std::io::Result<()> {
        let black_score = if let Ok(black_state) = self.black_state.read() {
            black_state.score
        } else {
            0
        };

        let white_score = if let Ok(white_state) = self.white_state.read() {
            white_state.score
        } else {
            0
        };

        let term = Term::stderr();

        if self.dirty {
            term.clear_last_lines(27)?;
            self.dirty = false;
        }

        term.write_line(&format!(
            "[{}/{}] Playing...",
            stats.finished_games() + 1,
            stats.total_games()
        ))?;
        term.write_line(&format!("{}", game.pos))?;
        term.write_line(&format!(
            "Time: (Black) {}s, (White) {}s",
            game.time.black_time().as_secs(),
            game.time.white_time().as_secs()
        ))?;
        term.write_line(&format!(
            "Score (Black) {black_score}, (White) {white_score}"
        ))?;
        self.dirty = true;

        Ok(())
    }

    fn on_game_over(
        &mut self,
        winner: Option<Color>,
        reason: GameOverReason,
        stats: &MatchStatistics,
    ) -> std::io::Result<()> {
        let term = Term::stderr();

        if self.dirty {
            term.clear_last_lines(27)?;
            self.dirty = false;
        }

        let result = match winner {
            Some(c) => {
                let name = if c == Color::Black { "Black" } else { "White" };
                format!("{name} won the game. ({reason:?})")
            }
            None => format!("Draw({reason:?})"),
        };
        term.write_line(&format!(
            "[{}/{}] {}",
            stats.finished_games() + 1,
            stats.total_games(),
            result
        ))?;

        Ok(())
    }
}

impl Reporter for BoardReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewTurn(ref game, _) => {
                self.on_new_turn(game, stats).unwrap();
            }
            Event::GameOver(winner, reason) => {
                self.on_game_over(winner, reason, stats).unwrap();
            }
            _ => {}
        }
    }
}
