use shogi::Color;
use std;
use std::io::Write;
use usi::GuiCommand;

use crate::player::EngineOutput;

use super::Reporter;

#[derive(Default)]
pub struct UsiReporter {}

impl Reporter for UsiReporter {
    fn on_send_command(&mut self, color: Color, _: &GuiCommand, raw_str: &str) {
        let prefix = if color == Color::Black { "B" } else { "W" };
        write!(&mut std::io::stderr(), "{}< {}", prefix, raw_str).unwrap();
    }

    fn on_receive_command(&mut self, color: Color, output: &EngineOutput) {
        let prefix = if color == Color::Black { "B" } else { "W" };
        write!(&mut std::io::stderr(), "{}> {}", prefix, output.raw_str()).unwrap();
    }
}
