use std::time::Instant;
use shogi::{Position, TimeControl};

#[derive(Debug)]
pub struct Game {
    pub black_player: String,
    pub white_player: String,
    pub pos: Position,
    pub time: TimeControl,
    pub turn_start_time: Instant,
}

impl Game {
    pub fn new(initial_time: TimeControl) -> Game {
        Game {
            black_player: String::new(),
            white_player: String::new(),
            pos: Position::new(),
            time: initial_time,
            turn_start_time: Instant::now(),
        }
    }
}