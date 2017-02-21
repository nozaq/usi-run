use std::time::Instant;
use shogi::{Position, TimeControl};

#[derive(Debug)]
pub struct Game {
    pub pos: Position,
    pub time: TimeControl,
    pub turn_start_time: Instant,
}

impl Game {
    pub fn new(initial_time: TimeControl) -> Game {
        Game {
            pos: Position::new(),
            time: initial_time,
            turn_start_time: Instant::now(),
        }
    }
}
