use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use environment::Event;
use shogi::Color;
use stats::MatchStatistics;

use super::Reporter;

#[derive(Default)]
pub struct SimpleReporter {
    current_bar: Option<ProgressBar>
}

impl Reporter for SimpleReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewGame(_) => {
                let current_game_num = stats.finished_games() + 1;
                let num_games = stats.total_games();

                let bar = ProgressBar::new_spinner();
                bar.set_draw_target(ProgressDrawTarget::stderr());
                bar.set_style(ProgressStyle::default_spinner()
                    .tick_chars("|/-\\ ")
                    .template("{prefix:.bold.dim} {spinner} {msg}"));
                bar.set_prefix(&format!("[{}/{}]", current_game_num, num_games));
                bar.set_message("Starting...");
                self.current_bar = Some(bar);
            },
            Event::NewTurn(ref game) => {
                if let Some(game) = game.read().ok() {
                    match self.current_bar {
                        Some(ref bar) => {
                            bar.set_message(&format!("Move #{}", game.pos.ply()));
                        },
                        None => {}
                    }
                }
            },
            Event::GameOver(winner, reason) => {
                match self.current_bar {
                    Some(ref bar) => {
                        let result = match winner {
                            Some(c) => {
                                let name = if c == Color::Black { "Black" } else { "White" };
                                format!("{} won the game. ({:?})", name, reason)
                            }
                            None => {
                                format!("Draw({:?})", reason)
                            }
                        };
                        bar.finish_with_message(&result);
                    },
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn on_match_finished(&mut self, stats: &MatchStatistics) {
        println!("{}\t{}\t{}\t{}", stats.finished_games(), stats.black_wins(), stats.white_wins(), stats.draw_games());
    }
}
