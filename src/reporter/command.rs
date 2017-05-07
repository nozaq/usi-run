use shogi::Color;
use shogi::usi::GuiCommand;
use usi::EngineOutput;

use super::Reporter;

pub struct UsiReporter {
}

impl Reporter for UsiReporter {
    fn on_send_command(&self, color: Color, _: &GuiCommand, raw_str: &str) {
        let prefix = if color == Color::Black { "B" } else { "W" };
        print!("{}< {}", prefix, raw_str);
    }

    fn on_receive_command(&self, color: Color, output: &EngineOutput) {
        let prefix = if color == Color::Black { "B" } else { "W" };
        print!("{}> {}", prefix, output.raw_str());
    }
}
