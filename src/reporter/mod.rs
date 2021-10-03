use crate::environment::Event;
use crate::stats::MatchStatistics;
use ::usi::{EngineOutput, GuiCommand};
use shogi::Color;

pub trait Reporter {
    fn on_send_command(&mut self, _stm: Color, _command: &GuiCommand, _arg: &str) {}
    fn on_receive_command(&mut self, _stm: Color, _output: &EngineOutput) {}
    fn on_game_event(&mut self, _event: &Event, _stats: &MatchStatistics) {}
    fn on_match_finished(&mut self, stats: &MatchStatistics) {
        println!("Total\tBlack\tWhite\tDraw");
        println!(
            "{}\t{}\t{}\t{}",
            stats.finished_games(),
            stats.black_wins(),
            stats.white_wins(),
            stats.draw_games()
        );
    }
}

mod board;
mod csa;
mod simple;
mod usi;

pub use self::board::*;
pub use self::csa::*;
pub use self::simple::*;
pub use self::usi::*;
