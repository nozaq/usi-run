use shogi::Color;

#[derive(Debug, Default)]
pub struct MatchStatistics {
    black_wins: u32,
    white_wins: u32,
    draw_games: u32,
    total_games: u32,
}

impl MatchStatistics {
    pub fn black_wins(&self) -> u32 {
        self.black_wins
    }

    pub fn white_wins(&self) -> u32 {
        self.white_wins
    }

    pub fn draw_games(&self) -> u32 {
        self.draw_games
    }

    pub fn total_games(&self) -> u32 {
        self.total_games
    }

    pub fn black_win_rate(&self) -> f32 {
        self.calc_rate(self.black_wins)
    }

    pub fn white_win_rate(&self) -> f32 {
        self.calc_rate(self.white_wins)
    }

    pub fn draw_game_rate(&self) -> f32 {
        self.calc_rate(self.draw_games)
    }

    pub fn record_game(&mut self, winner: Option<Color>) {
        if let Some(winner) = winner {
            if winner == Color::Black {
                self.black_wins += 1;
            } else {
                self.white_wins += 1;
            }
        } else {
            self.draw_games += 1;
        }
        self.total_games += 1;
    }

    fn calc_rate(&self, n: u32) -> f32 {
        (n as f32) / (self.total_games as f32)
    }
}