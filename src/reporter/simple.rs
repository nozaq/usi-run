use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::environment::Event;
use crate::stats::MatchStatistics;
use shogi::Color;

use super::Reporter;

#[derive(Default)]
pub struct SimpleReporter {
    current_bar: Option<ProgressBar>,
}

impl Reporter for SimpleReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewGame(_) => {
                let current_game_num = stats.finished_games() + 1;
                let num_games = stats.total_games();

                let pbar = ProgressBar::new_spinner();
                pbar.set_draw_target(ProgressDrawTarget::stderr());
                pbar.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("|/-\\ ")
                        .template("{prefix:.bold.dim} {spinner} {msg}")
                        .unwrap(),
                );
                pbar.set_prefix(format!("[{}/{}]", current_game_num, num_games));
                pbar.set_message("Starting...");
                self.current_bar = Some(pbar);
            }
            Event::NewTurn(ref game, _) => {
                if let Some(ref pbar) = self.current_bar {
                    pbar.set_message(format!("Move #{}", game.pos.ply()));
                }
            }
            Event::GameOver(winner, reason) => {
                if let Some(ref pbar) = self.current_bar {
                    let result = match winner {
                        Some(c) => {
                            let name = if c == Color::Black { "Black" } else { "White" };
                            format!("{} won the game. ({:?})", name, reason)
                        }
                        None => format!("Draw({:?})", reason),
                    };
                    pbar.finish_with_message(result);
                }
            }
            _ => {}
        }
    }
}
