use std;
use std::io::Write;

use environment::Event;
use stats::MatchStatistics;

use super::Reporter;

pub struct SimpleReporter {
}

impl Reporter for SimpleReporter {
    fn on_game_event(&self, event: &Event, _: &MatchStatistics) {
        match *event {            
            Event::GameOver(_, _) => {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            _ => {}
        }
    }
}
