use csa::{Action, Color, GameRecord, MoveRecord, PieceType, Square, Time};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::environment::Event;
use crate::game::GameOverReason;
use crate::stats::MatchStatistics;

use super::Reporter;

#[derive(Default)]
pub struct CsaReporter {
    current_bar: Option<ProgressBar>,
    record: GameRecord,
}

fn convert_color(c: shogi::Color) -> Color {
    match c {
        shogi::Color::Black => Color::Black,
        shogi::Color::White => Color::White,
    }
}

fn convert_pt(pt: shogi::PieceType) -> PieceType {
    match pt {
        shogi::PieceType::Pawn => PieceType::Pawn,
        shogi::PieceType::Lance => PieceType::Lance,
        shogi::PieceType::Knight => PieceType::Knight,
        shogi::PieceType::Silver => PieceType::Silver,
        shogi::PieceType::Gold => PieceType::Gold,
        shogi::PieceType::King => PieceType::King,
        shogi::PieceType::Rook => PieceType::Rook,
        shogi::PieceType::Bishop => PieceType::Bishop,
        shogi::PieceType::ProPawn => PieceType::ProPawn,
        shogi::PieceType::ProLance => PieceType::ProLance,
        shogi::PieceType::ProKnight => PieceType::ProKnight,
        shogi::PieceType::ProSilver => PieceType::ProSilver,
        shogi::PieceType::ProBishop => PieceType::Horse,
        shogi::PieceType::ProRook => PieceType::Dragon,
    }
}

fn convert_move_to_action(c: shogi::Color, m: &shogi::MoveRecord) -> Action {
    match *m {
        shogi::MoveRecord::Normal {
            from,
            to,
            placed: pc,
            promoted,
            ..
        } => {
            let pt = if promoted {
                pc.piece_type.promote().unwrap_or(pc.piece_type)
            } else {
                pc.piece_type
            };

            Action::Move(
                convert_color(c),
                Square::new(from.file() + 1, from.rank() + 1),
                Square::new(to.file() + 1, to.rank() + 1),
                convert_pt(pt),
            )
        }
        shogi::MoveRecord::Drop { to, piece: pc } => Action::Move(
            convert_color(c),
            Square::new(0, 0),
            Square::new(to.file() + 1, to.rank() + 1),
            convert_pt(pc.piece_type),
        ),
    }
}

impl Reporter for CsaReporter {
    fn on_game_event(&mut self, event: &Event, stats: &MatchStatistics) {
        match *event {
            Event::NewGame(ref game) => {
                let current_game_num = stats.finished_games() + 1;
                let num_games = stats.total_games();

                let pbar = ProgressBar::new_spinner();
                pbar.set_draw_target(ProgressDrawTarget::stderr());
                pbar.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("|/-\\ ")
                        .template("{prefix:.bold.dim} {spinner} {msg}")
                        .unwrap(),
                );
                pbar.set_prefix(format!("[{}/{}]", current_game_num, num_games));
                pbar.set_message("Starting...");
                self.current_bar = Some(pbar);

                self.record = GameRecord::default();

                self.record.black_player = Some(game.black_player.to_string());
                self.record.white_player = Some(game.white_player.to_string());

                self.record.start_time = Some(Time::now());
            }
            Event::NewTurn(ref game, elapsed) => {
                if let Some(last_move) = game.pos.move_history().last() {
                    self.record.moves.push(MoveRecord {
                        action: convert_move_to_action(game.pos.side_to_move().flip(), last_move),
                        time: Some(elapsed),
                    });
                }

                if let Some(ref pbar) = self.current_bar {
                    pbar.set_message(format!("Move #{}", game.pos.ply()));
                }
            }
            Event::GameOver(_, reason) => {
                let action = match reason {
                    GameOverReason::Resign => Action::Toryo,
                    GameOverReason::IllegalMove => Action::IllegalMove,
                    GameOverReason::OutOfTime => Action::TimeUp,
                    GameOverReason::MaxPly => Action::Hikiwake,
                    GameOverReason::DeclareWinning => Action::Kachi,
                };
                self.record.moves.push(MoveRecord { action, time: None });

                self.record.end_time = Some(Time::now());

                if let Some(ref pbar) = self.current_bar {
                    pbar.finish_and_clear();
                }

                if stats.finished_games() > 1 {
                    println!("/");
                }

                print!("{}", &self.record.to_string());
            }
            _ => {}
        }
    }

    fn on_match_finished(&mut self, _: &MatchStatistics) {}
}
