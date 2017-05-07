use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

use environment::Event;
use shogi::Color;
use stats::MatchStatistics;

use super::Reporter;

pub struct BoardReporter {
    black_score: Arc<AtomicIsize>,
    white_score: Arc<AtomicIsize>,
}

impl BoardReporter {
    pub fn new(black_score: Arc<AtomicIsize>, white_score: Arc<AtomicIsize>) -> BoardReporter {
        return BoardReporter{black_score, white_score};
    }
}

impl Reporter for BoardReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewTurn(ref game) => {
                if let Some(game) = game.read().ok() {
                    println!("{}", game.pos);
                    println!("Time: (Black) {}s, (White) {}s",
                                game.time.black_time().as_secs(),
                                game.time.white_time().as_secs());
                    println!("Score (Black) {}, (White) {}",
                                self.black_score.load(Ordering::Relaxed),
                                self.white_score.load(Ordering::Relaxed));
                    println!("");
                }
            }
            Event::GameOver(winner, reason) => {
                match winner {
                    Some(c) => {
                        let name = if c == Color::Black { "Black" } else { "White" };
                        println!("Game #{}: {} wins({:?})",
                                    stats.finished_games() + 1,
                                    name,
                                    reason);
                    }
                    None => {
                        println!("Game #{}: Draw({:?})", stats.finished_games() + 1, reason);
                    }
                }
            }
            _ => {}
        }
    }
}