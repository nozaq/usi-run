use environment::Event;
use shogi::Color;
use stats::MatchStatistics;
use shogi::usi::GuiCommand;
use usi::EngineOutput;

pub trait Reporter {
    fn on_send_command(&self, Color, &GuiCommand, &str) {}
    fn on_receive_command(&self, Color, &EngineOutput) {}
    fn on_game_event(&self, &Event, &MatchStatistics) {}
}

mod board;
mod command;
mod simple;

pub use self::board::*;
pub use self::command::*;
pub use self::simple::*;
