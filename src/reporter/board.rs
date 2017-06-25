use indicatif::Term;
use std;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

use environment::Event;
use shogi::Color;
use stats::MatchStatistics;

use environment::GameOverReason;
use game::Game;
use super::Reporter;

pub struct BoardReporter {
    dirty: bool,
    black_score: Arc<AtomicIsize>,
    white_score: Arc<AtomicIsize>,
}

impl BoardReporter {
    pub fn new(black_score: Arc<AtomicIsize>, white_score: Arc<AtomicIsize>) -> BoardReporter {
        return BoardReporter {
            dirty: false,
            black_score,
            white_score,
        };
    }

    fn on_new_turn(&mut self, game: &Game, stats: &MatchStatistics) -> std::io::Result<()> {
        let term = Term::stderr();

        if self.dirty {
            try!(term.clear_last_lines(27));
            self.dirty = false;
        }

        try!(term.write_line(&format!(
            "[{}/{}] Playing...",
            stats.finished_games() + 1,
            stats.total_games()
        )));
        try!(term.write_line(&format!("{}", game.pos)));
        try!(term.write_line(&format!(
            "Time: (Black) {}s, (White) {}s",
            game.time.black_time().as_secs(),
            game.time.white_time().as_secs()
        )));
        try!(term.write_line(&format!(
            "Score (Black) {}, (White) {}",
            self.black_score.load(Ordering::Relaxed),
            self.white_score.load(Ordering::Relaxed)
        )));
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
            try!(term.clear_last_lines(27));
            self.dirty = false;
        }

        let result = match winner {
            Some(c) => {
                let name = if c == Color::Black { "Black" } else { "White" };
                format!("{} won the game. ({:?})", name, reason)
            }
            None => format!("Draw({:?})", reason),
        };
        try!(term.write_line(&format!(
            "[{}/{}] {}",
            stats.finished_games() + 1,
            stats.total_games(),
            result
        )));

        Ok(())
    }
}

impl Reporter for BoardReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewTurn(ref game, _) => {
                if let Some(game) = game.read().ok() {
                    self.on_new_turn(&game, stats).unwrap();
                }
            }
            Event::GameOver(winner, reason) => {
                self.on_game_over(winner, reason, stats).unwrap();
            }
            _ => {}
        }
    }
}