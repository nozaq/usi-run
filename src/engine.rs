use shogi::{Color, Move, SfenError, TimeControl};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use usi::{
    BestMoveParams, EngineCommand, EngineOutput, GameOverKind, GuiCommand, InfoParams, ScoreKind,
    ThinkParams, UsiEngineHandler,
};

use crate::environment::*;
use crate::error::Error;
use crate::EngineConfig;

pub type ReadHookFn = Box<dyn FnMut(&EngineOutput) -> Result<(), Error> + Send>;
pub type WriteHookFn = Box<dyn FnMut(&GuiCommand, &str) + Send>;

#[derive(Default)]
pub struct ThinkState {
    pub score: i32,
    pondering: Option<Move>,
    pending: Option<()>,
}

pub struct UsiEngine {
    pub color: Color,
    pub name: String,
    handler: UsiEngineHandler,
    think_state: Arc<RwLock<ThinkState>>,
}

impl UsiEngine {
    pub fn new(
        color: Color,
        config: &EngineConfig,
        action_out: Sender<Action>,
        mut read_hook: Option<ReadHookFn>,
        think_state: Arc<RwLock<ThinkState>>,
    ) -> Result<UsiEngine, Error> {
        let mut handler = UsiEngineHandler::spawn(&config.engine_path, &config.working_dir)?;
        handler.prepare()?;

        let info = handler.get_info()?;

        let mut options = info.options().clone();
        for (name, value) in &config.options {
            options.insert(name.to_string(), value.to_string());
        }
        options.insert("USI_Ponder".to_string(), config.ponder.to_string());

        for (name, value) in &options {
            handler.send_command(&GuiCommand::SetOption(
                name.to_string(),
                Some(value.to_string()),
            ))?;
        }

        handler.listen({
            let ponder = config.ponder;
            let think_state = think_state.clone();

            move |output: &EngineOutput| -> Result<(), Error> {
                match output.response() {
                    Some(EngineCommand::ReadyOk) => {
                        action_out.send(Action::Ready(color))?;
                    }
                    Some(EngineCommand::BestMove(BestMoveParams::MakeMove(
                        best_move_sfen,
                        ponder_move,
                    ))) => {
                        if let Ok(mut think_state) = think_state.write() {
                            if think_state.pending.is_some() {
                                action_out.send(Action::RequestState)?;
                            } else {
                                if ponder {
                                    if let Some(ref ponder_move) = *ponder_move {
                                        if let Some(ponder_move) = Move::from_sfen(ponder_move) {
                                            think_state.pondering = Some(ponder_move);
                                        }
                                    }
                                }

                                if let Some(best_move) = Move::from_sfen(best_move_sfen) {
                                    action_out.send(Action::MakeMove(
                                        color,
                                        best_move,
                                        *output.timestamp(),
                                    ))?;
                                } else {
                                    return Err(Error::Sfen(SfenError::IllegalMove));
                                }
                            }
                        }
                    }
                    Some(EngineCommand::BestMove(BestMoveParams::Resign)) => {
                        action_out.send(Action::Resign(color))?;
                    }
                    Some(EngineCommand::BestMove(BestMoveParams::Win)) => {
                        action_out.send(Action::DeclareWinning(color))?;
                    }
                    Some(EngineCommand::Info(v)) => {
                        if let Ok(mut think_state) = think_state.write() {
                            if let Some(InfoParams::Score(val, ScoreKind::CpExact)) = v
                                .iter()
                                .find(|item| matches!(*(*item), InfoParams::Score(_, _)))
                            {
                                think_state.score = *val;
                            }
                        }
                    }
                    _ => {}
                }

                if let Some(ref mut f) = read_hook {
                    f(output)?;
                }

                Ok(())
            }
        })?;
        let engine = UsiEngine {
            color,
            name: info.name().to_string(),
            handler,
            think_state,
        };

        Ok(engine)
    }

    pub fn notify_event(
        &mut self,
        event: &Event,
        hook: &mut Option<WriteHookFn>,
    ) -> Result<(), Error> {
        let mut write = {
            let handler = &mut self.handler;

            move |cmd: &GuiCommand| -> Result<(), Error> {
                handler.send_command(cmd)?;
                if let Some(ref mut f) = hook {
                    f(cmd, &cmd.to_string());
                }
                Ok(())
            }
        };

        match event {
            Event::IsReady => {
                write(&GuiCommand::IsReady)?;
            }
            Event::NewGame(_) => {
                if let Ok(mut think_state) = self.think_state.write() {
                    think_state.score = 0;
                    write(&GuiCommand::UsiNewGame)?;
                }
            }
            Event::NewTurn(game, _) => {
                if let Ok(mut think_state) = self.think_state.write() {
                    if game.pos.side_to_move() == self.color {
                        if let Some(ponder_move) = think_state.pondering {
                            if let Some(last) = game.pos.move_history().last() {
                                if *last == ponder_move {
                                    write(&GuiCommand::Ponderhit)?;
                                } else {
                                    write(&GuiCommand::Stop)?;

                                    think_state.pending = Some(());
                                }
                                return Ok(());
                            }
                        }

                        let sfen = game.pos.to_sfen();
                        write(&GuiCommand::Position(sfen))?;
                        write(&GuiCommand::Go(build_think_params(&game.time)))?;
                    } else if let Some(ponder_move) = think_state.pondering {
                        let sfen = game.pos.to_sfen();
                        write(&GuiCommand::Position(format!("{sfen} {ponder_move}")))?;
                        write(&GuiCommand::Go(build_think_params(&game.time).ponder()))?;
                    }
                }
            }
            Event::NotifyState(game) => {
                if let Ok(mut think_state) = self.think_state.write() {
                    if game.pos.side_to_move() == self.color {
                        think_state.pending = None;
                        let sfen = game.pos.to_sfen();
                        write(&GuiCommand::Position(sfen))?;
                        write(&GuiCommand::Go(build_think_params(&game.time)))?;
                    }
                }
            }
            Event::GameOver(winner, _) => {
                let result = match winner {
                    Some(c) if *c == self.color => GameOverKind::Win,
                    Some(_) => GameOverKind::Lose,
                    None => GameOverKind::Draw,
                };
                write(&GuiCommand::Stop)?;
                write(&GuiCommand::GameOver(result))?;
            }
        }
        Ok(())
    }
}

fn build_think_params(time: &TimeControl) -> ThinkParams {
    match *time {
        TimeControl::Byoyomi {
            black_time,
            white_time,
            byoyomi,
        } => ThinkParams::new()
            .btime(black_time)
            .wtime(white_time)
            .byoyomi(byoyomi),
        TimeControl::FischerClock {
            black_time,
            white_time,
            black_inc,
            white_inc,
        } => ThinkParams::new()
            .btime(black_time)
            .wtime(white_time)
            .binc(black_inc)
            .winc(white_inc),
    }
}
