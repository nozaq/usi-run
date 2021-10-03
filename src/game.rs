use shogi::{Color, Position, TimeControl};
use std::time::Instant;

#[derive(Debug)]
pub struct Game {
    pub black_player: String,
    pub white_player: String,
    pub pos: Position,
    pub time: TimeControl,
    pub turn_start_time: Instant,
}

#[derive(Debug, Clone)]
pub struct GameResult {
    pub winner: Option<Color>,
    pub reason: GameOverReason,
}

impl GameResult {
    pub fn new(winner: Option<Color>, reason: GameOverReason) -> GameResult {
        GameResult { winner, reason }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GameOverReason {
    Resign,
    IllegalMove,
    OutOfTime,
    MaxPly,
    DeclareWinning,
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
