use shogi::{Color, Move, SfenError, TimeControl};
use std::collections::HashMap;
use std::io;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use usi::*;

use super::reader::*;
use super::writer::*;
use crate::environment::*;
use crate::error::Error;
use crate::EngineConfig;

pub type WriteHookFn = Box<dyn FnMut(&GuiCommand, &str) + Send + Sync>;
pub type ReadHookFn = Box<dyn FnMut(&EngineOutput) + Send + Sync>;

pub struct UsiEngine {
    pub color: Color,
    pub name: String,
    pub score: Arc<AtomicIsize>,
    ponder: bool,
    process: Child,
    reader: Arc<Mutex<EngineCommandReader<ChildStdout>>>,
    writer: Arc<Mutex<GuiCommandWriter<ChildStdin>>>,
    pondering: Arc<Mutex<Option<Move>>>,
    pending: Arc<Mutex<Option<()>>>,
    write_hook: Arc<Mutex<Option<WriteHookFn>>>,
    read_hook: Arc<Mutex<Option<ReadHookFn>>>,
}

impl UsiEngine {
    pub fn launch(color: Color, config: &EngineConfig) -> Result<UsiEngine, Error> {
        let mut process = Command::new(&config.engine_path)
            .current_dir(&config.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        let mut w = GuiCommandWriter::new(stdin);
        let mut r = EngineCommandReader::new(stdout);

        let mut engine_name = String::new();
        let mut opts = HashMap::new();
        w.send(&GuiCommand::Usi)?;
        loop {
            let output = r.next()?;
            match *output.response() {
                Some(EngineCommand::Id(IdParams::Name(ref name))) => {
                    engine_name = name.to_string();
                }
                Some(EngineCommand::Option(OptionParams {
                    ref name,
                    ref value,
                })) => {
                    opts.insert(
                        name.to_string(),
                        match *value {
                            OptionKind::Check { default: Some(f) } => {
                                if f { "true" } else { "false" }.to_string()
                            }
                            OptionKind::Spin {
                                default: Some(ref n),
                                ..
                            } => n.to_string(),
                            OptionKind::Combo {
                                default: Some(ref s),
                                ..
                            } => s.to_string(),
                            OptionKind::Button {
                                default: Some(ref s),
                            } => s.to_string(),
                            OptionKind::String {
                                default: Some(ref s),
                            } => s.to_string(),
                            OptionKind::Filename {
                                default: Some(ref s),
                            } => s.to_string(),
                            _ => String::new(),
                        },
                    );
                }
                Some(EngineCommand::UsiOk) => break,
                _ => {}
            }
        }

        for &(ref name, ref value) in &config.options {
            opts.insert(name.to_string(), value.to_string());
        }

        for (name, value) in &opts {
            w.send(&GuiCommand::SetOption(
                name.to_string(),
                Some(value.to_string()),
            ))?;
        }

        if config.ponder {
            w.send(&GuiCommand::SetOption(
                "USI_Ponder".to_string(),
                Some(config.ponder.to_string()),
            ))?;
        }

        Ok(UsiEngine {
            color,
            name: engine_name,
            ponder: config.ponder,
            process,
            reader: Arc::new(Mutex::new(r)),
            writer: Arc::new(Mutex::new(w)),
            pondering: Default::default(),
            pending: Default::default(),
            score: Default::default(),
            write_hook: Default::default(),
            read_hook: Default::default(),
        })
    }

    pub fn set_write_hook(&mut self, f: Option<WriteHookFn>) {
        self.write_hook = Arc::new(Mutex::new(f));
    }

    pub fn set_read_hook(&mut self, f: Option<ReadHookFn>) {
        self.read_hook = Arc::new(Mutex::new(f));
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.process.kill()
    }

    pub fn listen(&mut self, action_out: Sender<Action>, event_in: Receiver<Event>) {
        self.listen_commands(action_out);
        self.listen_events(event_in);
    }

    fn listen_commands(&mut self, action_out: Sender<Action>) {
        let color = self.color;
        let ponder = self.ponder;
        let reader = self.reader.clone();
        let pending = self.pending.clone();
        let pondering = self.pondering.clone();
        let score = self.score.clone();
        let hook = self.read_hook.clone();

        // Engine to Game.
        thread::spawn(move || -> Result<(), Error> {
            let mut reader = reader.lock().unwrap();
            let mut hook = hook.lock().unwrap();

            loop {
                match reader.next() {
                    Ok(output) => {
                        match *output.response() {
                            Some(EngineCommand::ReadyOk) => {
                                action_out.send(Action::Ready(color))?;
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::MakeMove(
                                ref best_move_sfen,
                                ref ponder_move,
                            ))) => {
                                if let Ok(pending) = pending.lock() {
                                    if pending.is_some() {
                                        action_out.send(Action::RequestState)?;
                                        continue;
                                    }
                                }

                                if ponder {
                                    if let Ok(mut guard) = pondering.lock() {
                                        if let Some(ref ponder_move) = *ponder_move {
                                            if let Some(ponder_move) = Move::from_sfen(ponder_move)
                                            {
                                                *guard = Some(ponder_move);
                                            }
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
                                    return Err(Error::Sfen(SfenError {}));
                                }
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::Resign)) => {
                                action_out.send(Action::Resign(color))?;
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::Win)) => {
                                action_out.send(Action::DeclareWinning(color))?;
                            }
                            Some(EngineCommand::Info(ref v)) => {
                                if let Some(InfoParams::Score(val, ScoreKind::CpExact)) = v
                                    .iter()
                                    .find(|item| matches!(*(*item), InfoParams::Score(_, _)))
                                {
                                    score.store(*val as isize, Ordering::Relaxed)
                                }
                            }
                            _ => {}
                        }

                        if let Some(ref mut f) = *hook {
                            f(&output);
                        }
                    }
                    Err(Error::Usi(_)) => {
                        // Ignore illegal commands.
                        continue;
                    }
                    Err(err) => {
                        println!("{}", err);
                        // TODO error handling?
                        break;
                    }
                }
            }
            Ok(())
        });
    }

    fn listen_events(&mut self, event_in: Receiver<Event>) {
        let color = self.color;
        let writer = self.writer.clone();
        let pending = self.pending.clone();
        let pondering = self.pondering.clone();
        let score = self.score.clone();
        let hook = self.write_hook.clone();

        // Game to Engine.
        thread::spawn(move || -> Result<(), Error> {
            let mut writer = writer.lock().unwrap();
            let mut hook = hook.lock().unwrap();

            let mut write = |cmd: &GuiCommand| -> Result<(), Error> {
                let s = writer.send(cmd)?;
                if let Some(ref mut f) = *hook {
                    f(cmd, &s);
                }
                Ok(())
            };

            while let Ok(event) = event_in.recv() {
                match event {
                    Event::IsReady => {
                        write(&GuiCommand::IsReady)?;
                    }
                    Event::NewGame(_) => {
                        score.store(0, Ordering::Relaxed);
                        write(&GuiCommand::UsiNewGame)?;
                    }
                    Event::NewTurn(shared_game, _) => {
                        if let Ok(game) = shared_game.read() {
                            if game.pos.side_to_move() == color {
                                if let Ok(guard) = pondering.lock() {
                                    if let Some(ponder_move) = *guard {
                                        if let Some(last) = game.pos.move_history().last() {
                                            if *last == ponder_move {
                                                write(&GuiCommand::Ponderhit)?;
                                            } else {
                                                write(&GuiCommand::Stop)?;

                                                if let Ok(mut guard2) = pending.lock() {
                                                    *guard2 = Some(());
                                                }
                                            }
                                            continue;
                                        }
                                    }
                                }

                                let sfen = game.pos.to_sfen();
                                write(&GuiCommand::Position(sfen))?;
                                write(&GuiCommand::Go(build_think_params(&game.time)))?;
                            } else if let Ok(guard) = pondering.lock() {
                                if let Some(ponder_move) = *guard {
                                    let sfen = game.pos.to_sfen();
                                    write(&GuiCommand::Position(format!(
                                        "{} {}",
                                        sfen, ponder_move
                                    )))?;
                                    write(&GuiCommand::Go(
                                        build_think_params(&game.time).ponder(),
                                    ))?;
                                }
                            }
                        }
                    }
                    Event::NotifyState(shared_game) => {
                        if let Ok(game) = shared_game.read() {
                            if game.pos.side_to_move() == color {
                                if let Ok(mut data) = pending.lock() {
                                    *data = None;
                                    let sfen = game.pos.to_sfen();
                                    write(&GuiCommand::Position(sfen.to_string()))?;
                                    write(&GuiCommand::Go(build_think_params(&game.time)))?;
                                }
                            }
                        }
                    }
                    Event::GameOver(winner, _) => {
                        let result = match winner {
                            Some(c) if c == color => GameOverKind::Win,
                            Some(_) => GameOverKind::Lose,
                            None => GameOverKind::Draw,
                        };
                        write(&GuiCommand::Stop)?;
                        write(&GuiCommand::GameOver(result))?;
                    }
                }
            }
            Ok(())
        });
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
