use environment::Event;
use shogi::Color;
use stats::MatchStatistics;
use shogi::usi::GuiCommand;
use usi::EngineOutput;

pub trait Reporter {
    fn on_send_command(&mut self, Color, &GuiCommand, &str) {}
    fn on_receive_command(&mut self, Color, &EngineOutput) {}
    fn on_game_event(&mut self, &Event, &MatchStatistics) {}
    fn on_match_finished(&mut self, &MatchStatistics) {}
}

mod board;
mod command;
mod simple;

pub use self::board::*;
pub use self::command::*;
pub use self::simple::*;
