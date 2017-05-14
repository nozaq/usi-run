use std::collections::HashMap;
use std::io;
use std::process::{Command, Stdio, Child, ChildStdin, ChildStdout};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Sender, Receiver};
use std::thread;
use shogi::{Color, Move, TimeControl, SfenError};
use usi::*;

use EngineConfig;
use error::Error;
use environment::*;
use super::reader::*;
use super::writer::*;

pub struct UsiEngine {
    pub color: Color,
    pub score: Arc<AtomicIsize>,
    ponder: bool,
    process: Child,
    reader: Arc<Mutex<EngineCommandReader<ChildStdout>>>,
    writer: Arc<Mutex<GuiCommandWriter<ChildStdin>>>,
    pondering: Arc<Mutex<Option<Move>>>,
    pending: Arc<Mutex<Option<()>>>,
    write_hook: Arc<Mutex<Option<Box<FnMut(&GuiCommand, &str) + Send + Sync>>>>,
    read_hook: Arc<Mutex<Option<Box<FnMut(&EngineOutput) + Send + Sync>>>>,
}

impl UsiEngine {
    pub fn launch(color: Color, config: &EngineConfig) -> Result<UsiEngine, Error> {
        let mut process = try!(Command::new(&config.engine_path)
            .current_dir(&config.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn());

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        let mut w = GuiCommandWriter::new(stdin);
        let mut r = EngineCommandReader::new(stdout);

        let mut opts = HashMap::new();
        try!(w.send(&GuiCommand::Usi));
        loop {
            let output = try!(r.next());
            match *output.response() {
                Some(EngineCommand::Option(OptionParams { ref name, ref value })) => {
                    opts.insert(name.to_string(),
                                match value {
                                    &OptionKind::Check { default: Some(f) } => {
                                        if f { "true" } else { "false" }.to_string()
                                    }
                                    &OptionKind::Spin { default: Some(ref n), .. } => n.to_string(),
                                    &OptionKind::Combo { default: Some(ref s), .. } => {
                                        s.to_string()
                                    }
                                    &OptionKind::Button { default: Some(ref s) } => s.to_string(),
                                    &OptionKind::String { default: Some(ref s) } => s.to_string(),
                                    &OptionKind::Filename { default: Some(ref s) } => s.to_string(),
                                    _ => String::new(),
                                });
                }
                Some(EngineCommand::UsiOk) => break,
                _ => {}
            }
        }

        for &(ref name, ref value) in &config.options {
            opts.insert(name.to_string(), value.to_string());
        }

        for (name, value) in &opts {
            try!(w.send(&GuiCommand::SetOption(name.to_string(), Some(value.to_string()))));
        }

        if config.ponder {
            try!(w.send(&GuiCommand::SetOption("USI_Ponder".to_string(),
                                               Some(config.ponder.to_string()))));
        }

        Ok(UsiEngine {
            color: color,
            ponder: config.ponder,
            process: process,
            reader: Arc::new(Mutex::new(r)),
            writer: Arc::new(Mutex::new(w)),
            pondering: Default::default(),
            pending: Default::default(),
            score: Default::default(),
            write_hook: Default::default(),
            read_hook: Default::default(),
        })
    }

    pub fn set_write_hook(&mut self, f: Option<Box<FnMut(&GuiCommand, &str) + Send + Sync>>) {
        self.write_hook = Arc::new(Mutex::new(f));
    }

    pub fn set_read_hook(&mut self, f: Option<Box<FnMut(&EngineOutput) + Send + Sync>>) {
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
        let color = self.color.clone();
        let ponder = self.ponder.clone();
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
                                try!(action_out.send(Action::Ready(color)));
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::MakeMove(ref best_move_sfen,
                                                                         ref ponder_move))) => {
                                if let Some(pending) = pending.lock().ok() {
                                    if let Some(_) = *pending {
                                        try!(action_out.send(Action::RequestState));
                                        continue;
                                    }
                                }

                                if ponder {
                                    if let Some(mut guard) = pondering.lock().ok() {
                                        if let Some(ref ponder_move) = *ponder_move {
                                            if let Some(ponder_move) = Move::from_sfen(ponder_move) {
                                                *guard = Some(ponder_move);
                                            }
                                        }
                                    }
                                }

                                if let Some(best_move) = Move::from_sfen(best_move_sfen) {
                                    try!(action_out.send(Action::MakeMove(color, best_move, *output.timestamp())));
                                } else {
                                    return Err(Error::Sfen(SfenError{}));
                                }
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::Resign)) => {
                                try!(action_out.send(Action::Resign(color)));
                            }
                            Some(EngineCommand::BestMove(BestMoveParams::Win)) => {
                                try!(action_out.send(Action::DeclareWinning(color)));
                            }
                            Some(EngineCommand::Info(ref v)) => {
                                if let Some(score_entry) =
                                    v.iter().find(|item| match *item {
                                        &InfoParams::Score(_, _) => true,
                                        _ => false,
                                    }) {
                                    match *score_entry {
                                        InfoParams::Score(val, ScoreKind::CpExact) => {
                                            score.store(val as isize, Ordering::Relaxed)
                                        }
                                        _ => {}
                                    }
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
        let color = self.color.clone();
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
                let s = try!(writer.send(cmd));
                if let Some(ref mut f) = *hook {
                    f(cmd, &s);
                }
                Ok(())
            };

            while let Some(event) = event_in.recv().ok() {
                match event {
                    Event::IsReady => {
                        try!(write(&GuiCommand::IsReady));
                    }
                    Event::NewGame(_) => {
                        score.store(0, Ordering::Relaxed);
                        try!(write(&GuiCommand::UsiNewGame));
                    }
                    Event::NewTurn(shared_game) => {
                        if let Some(game) = shared_game.read().ok() {
                            if game.pos.side_to_move() == color {
                                if let Some(guard) = pondering.lock().ok() {
                                    if let Some(ponder_move) = *guard {
                                        if let Some(last) = game.pos.move_history().last() {
                                            if *last == ponder_move {
                                                try!(write(&GuiCommand::Ponderhit));
                                                continue;
                                            } else {
                                                try!(write(&GuiCommand::Stop));

                                                if let Some(mut guard2) = pending.lock()
                                                    .ok() {
                                                    *guard2 = Some(());
                                                }

                                                continue;
                                            }
                                        }
                                    }
                                }

                                let sfen = game.pos.to_sfen();
                                try!(write(&GuiCommand::Position(sfen)));
                                try!(write(&GuiCommand::Go(build_think_params(&game.time))));
                            } else {
                                if let Some(guard) = pondering.lock().ok() {
                                    if let Some(ponder_move) = *guard {
                                        let sfen = game.pos.to_sfen();
                                        try!(write(&GuiCommand::Position(format!("{} {}",
                                                                                 sfen,
                                                                                 ponder_move))));
                                        try!(write(&GuiCommand::Go(build_think_params(&game.time)
                                            .ponder())));
                                    }
                                }
                            }
                        }
                    }
                    Event::NotifyState(shared_game) => {
                        if let Some(game) = shared_game.read().ok() {
                            if game.pos.side_to_move() == color {
                                if let Some(mut data) = pending.lock().ok() {
                                    *data = None;
                                    let sfen = game.pos.to_sfen();
                                    try!(write(&GuiCommand::Position(sfen.to_string())));
                                    try!(write(&GuiCommand::Go(build_think_params(&game.time))));
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
                        try!(write(&GuiCommand::Stop));
                        try!(write(&GuiCommand::GameOver(result)));
                    }
                }

            }
            Ok(())
        });
    }
}

fn build_think_params(time: &TimeControl) -> ThinkParams {
    match *time {
        TimeControl::Byoyomi { black_time, white_time, byoyomi } => {
            ThinkParams::new().btime(black_time).wtime(white_time).byoyomi(byoyomi)
        }
        TimeControl::FischerClock { black_time, white_time, black_inc, white_inc } => {
            ThinkParams::new().btime(black_time).wtime(white_time).binc(black_inc).winc(white_inc)
        }
    }
}