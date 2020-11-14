use shogi::TimeControl;
use std::fs::File;
use std::io::{Error, Read};
use std::time::Duration;
use toml::Value;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DisplayMode {
    Board,
    Command,
    Csa,
    Simple,
}

#[derive(Debug, Default)]
pub struct EngineConfig {
    pub engine_path: String,
    pub working_dir: String,
    pub ponder: bool,
    pub options: Vec<(String, String)>,
}

impl EngineConfig {
    fn merge(&mut self, value: &Value) {
        if let Some(engine_path) = value.get("engine_path").and_then(|v| v.as_str()) {
            self.engine_path = engine_path.to_string();
        }

        if let Some(working_dir) = value.get("working_dir").and_then(|v| v.as_str()) {
            self.working_dir = working_dir.to_string();
        }

        if let Some(flag) = value.get("ponder").and_then(|v| v.as_bool()) {
            self.ponder = flag;
        }

        if let Some(options) = value.get("options").and_then(|v| v.as_table()) {
            for (name, value) in options.iter() {
                self.options.push((name.to_string(), value.to_string()));
            }
        }
    }
}

#[derive(Debug)]
pub struct TimeControlConfig {
    pub black_time: Duration,
    pub white_time: Duration,
    pub byoyomi: Option<Duration>,
    pub black_inc: Option<Duration>,
    pub white_inc: Option<Duration>,
}

impl TimeControlConfig {
    fn merge(&mut self, value: &Value) {
        if let Some(btime) = value.get("black_time").and_then(|v| v.as_integer()) {
            self.black_time = Duration::from_millis(btime as u64);
        }

        if let Some(wtime) = value.get("white_time").and_then(|v| v.as_integer()) {
            self.white_time = Duration::from_millis(wtime as u64);
        }

        self.byoyomi = value
            .get("byoyomi")
            .and_then(|v| v.as_integer())
            .map(|v| Duration::from_millis(v as u64));

        self.black_inc = value
            .get("black_inc")
            .and_then(|v| v.as_integer())
            .map(|v| Duration::from_millis(v as u64));

        self.white_inc = value
            .get("white_inc")
            .and_then(|v| v.as_integer())
            .map(|v| Duration::from_millis(v as u64));
    }

    pub fn to_time_control(&self) -> TimeControl {
        if let Some(byoyomi) = self.byoyomi {
            TimeControl::Byoyomi {
                black_time: self.black_time,
                white_time: self.white_time,
                byoyomi,
            }
        } else {
            TimeControl::FischerClock {
                black_time: self.black_time,
                white_time: self.white_time,
                black_inc: self.black_inc.unwrap_or_else(|| Duration::from_secs(0)),
                white_inc: self.white_inc.unwrap_or_else(|| Duration::from_secs(0)),
            }
        }
    }
}

impl Default for TimeControlConfig {
    fn default() -> TimeControlConfig {
        // Default values are derived from the rules of WCSC26.
        // http://www.computer-shogi.org/wcsc26/
        TimeControlConfig {
            black_time: Duration::from_secs(600),
            white_time: Duration::from_secs(600),
            byoyomi: None,
            black_inc: Some(Duration::from_secs(10)),
            white_inc: Some(Duration::from_secs(10)),
        }
    }
}

#[derive(Debug)]
pub struct MatchConfig {
    pub num_games: u32,
    pub max_ply: Option<u16>,
    pub initial_pos: Option<String>,
    pub black_engine: EngineConfig,
    pub white_engine: EngineConfig,
    pub time: TimeControlConfig,
    pub display: DisplayMode,
}

impl MatchConfig {
    pub fn load(&mut self, config_path: &str) -> Result<(), Error> {
        let mut f = File::open(config_path)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;

        let value = buf.parse::<Value>().unwrap();

        self.num_games = value
            .get("num_games")
            .and_then(|v| v.as_integer())
            .map(|v| v as u32)
            .unwrap_or(1);
        self.max_ply = value
            .get("max_ply")
            .and_then(|v| v.as_integer())
            .map(|v| v as u16);
        self.initial_pos = value
            .get("initial_pos")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        if let Some(black) = value.get("black") {
            self.black_engine.merge(&black);
        }

        if let Some(white) = value.get("white") {
            self.white_engine.merge(&white);
        }

        if let Some(time_control) = value.get("time_control") {
            self.time.merge(&time_control);
        }

        Ok(())
    }
}

impl Default for MatchConfig {
    fn default() -> MatchConfig {
        // Default values are derived from the rules of WCSC26.
        // http://www.computer-shogi.org/wcsc26/
        MatchConfig {
            num_games: 1,
            max_ply: Some(256),
            initial_pos: None,
            black_engine: Default::default(),
            white_engine: Default::default(),
            time: Default::default(),
            display: DisplayMode::Board,
        }
    }
}
