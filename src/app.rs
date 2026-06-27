use crate::ai;
use crate::analysis::{Analysis, winrate_from_lead};
use crate::config::Config;
use crate::game::{Game, Play, Stone, play_name};
use crate::gtp::{GtpEngine, engine_installed};
use crate::sgf;
use crate::theme::{Theme, theme_at};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Menu,
    Setup,
    Playing,
    Review,
    Browser,
    Themes,
    Help,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Engine,
    Friend,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EngineChoice {
    Builtin,
    External,
}

#[derive(Clone, Copy)]
pub struct Setup {
    pub board_size: usize,
    pub mode: Mode,
    pub engine: EngineChoice,
    pub level: u8,
    pub color: Stone,
    pub handicap: u8,
    pub komi: f32,
    pub main_time: Option<u64>,
    pub increment: u64,
    pub hints: u32,
    pub undo: bool,
    pub eval: bool,
    pub row: usize,
}

pub const SETUP_ROWS: usize = 12;

impl Setup {
    pub fn from_config(config: &Config) -> Setup {
        Setup {
            board_size: config.board_size,
            mode: Mode::Engine,
            engine: EngineChoice::External,
            level: config.level,
            color: Stone::Black,
            handicap: 0,
            komi: config.komi,
            main_time: None,
            increment: 0,
            hints: 3,
            undo: true,
            eval: config.show_eval,
            row: 0,
        }
    }

    pub fn value(&self, row: usize, engine_name: &str, engine_ready: bool) -> String {
        match row {
            0 => format!("{0}x{0}", self.board_size),
            1 => match self.mode {
                Mode::Engine => "computer".to_string(),
                Mode::Friend => "a friend, same keyboard".to_string(),
            },
            2 => match self.engine {
                EngineChoice::Builtin => "built-in".to_string(),
                EngineChoice::External => {
                    if engine_ready {
                        engine_name.to_string()
                    } else {
                        format!("{} (not installed)", engine_name)
                    }
                }
            },
            3 => format!("{} of 10", self.level),
            4 => self.color.name().to_string(),
            5 => {
                if self.handicap == 0 {
                    "none".to_string()
                } else {
                    format!("{} stones", self.handicap)
                }
            }
            6 => format!("{}", self.komi),
            7 => match self.main_time {
                None => "no clock".to_string(),
                Some(seconds) => format!("{} min", seconds / 60),
            },
            8 => format!("{} s", self.increment),
            9 => match self.hints {
                0 => "off".to_string(),
                u32::MAX => "unlimited".to_string(),
                n => format!("{}", n),
            },
            10 => on_off(self.undo),
            _ => on_off(self.eval),
        }
    }

    pub fn active(&self, row: usize) -> bool {
        match row {
            2 | 3 | 9 => self.mode == Mode::Engine,
            4 => self.mode == Mode::Engine,
            _ => true,
        }
    }

    pub fn change(&mut self, forward: bool) {
        match self.row {
            0 => self.board_size = cycle(&[9, 13, 19], self.board_size, forward),
            1 => {
                self.mode = if self.mode == Mode::Engine {
                    Mode::Friend
                } else {
                    Mode::Engine
                }
            }
            2 => {
                self.engine = if self.engine == EngineChoice::Builtin {
                    EngineChoice::External
                } else {
                    EngineChoice::Builtin
                }
            }
            3 => self.level = step(self.level, 1, 10, forward),
            4 => self.color = self.color.other(),
            5 => self.handicap = cycle(&[0, 2, 3, 4, 5, 6, 7, 8, 9], self.handicap, forward),
            6 => self.komi = cycle(&[0.0, 0.5, 5.5, 6.5, 7.5], self.komi, forward),
            7 => {
                self.main_time = cycle(
                    &[None, Some(180), Some(300), Some(600), Some(1200), Some(1800)],
                    self.main_time,
                    forward,
                )
            }
            8 => self.increment = cycle(&[0, 3, 5, 10, 30], self.increment, forward),
            9 => self.hints = cycle(&[0, 3, 5, 10, u32::MAX], self.hints, forward),
            10 => self.undo = !self.undo,
            _ => self.eval = !self.eval,
        }
    }
}

pub const SETUP_LABELS: [&str; SETUP_ROWS] = [
    "Board",
    "Play against",
    "Engine",
    "Strength",
    "You play",
    "Handicap",
    "Komi",
    "Main time",
    "Increment",
    "Hints",
    "Undo",
    "Eval bar",
];

#[derive(Clone)]
enum Brain {
    Builtin(u8),
    External(Arc<Mutex<GtpEngine>>),
}

pub struct Session {
    pub game: Game,
    pub mode: Mode,
    pub human: Stone,
    pub cursor: usize,
    pub status: String,
    pub plays: Vec<(Stone, Play)>,
    pub positions: Vec<Game>,
    pub winrates: Vec<Option<f32>>,
    pub last: Option<usize>,
    pub clocks: [Option<f64>; 2],
    pub increment: f64,
    pub hints_left: u32,
    pub hint: Option<Play>,
    pub show_territory: bool,
    pub territory: Option<Vec<Option<Stone>>>,
    pub undo_enabled: bool,
    pub show_eval: bool,
    pub thinking: bool,
    pub hint_pending: bool,
    pub engine_name: String,
    pub result: String,

    brain: Brain,
    move_box: Option<Receiver<Play>>,
    hint_box: Option<Receiver<Play>>,
    eval_box: Option<Receiver<(usize, f32)>>,
    eval_due: bool,
    ticked: Instant,
}

impl Session {
    pub fn new(setup: &Setup, config: &Config, engine_ready: bool) -> Session {
        let mut game = Game::new(setup.board_size, setup.komi);
        if setup.handicap > 0 {
            game.place_handicap(setup.handicap);
        }

        let external = if setup.mode == Mode::Engine
            && setup.engine == EngineChoice::External
            && engine_ready
        {
            GtpEngine::start(&config.engine_command, setup.board_size, setup.komi)
                .map(|engine| Arc::new(Mutex::new(engine)))
        } else {
            None
        };

        if let Some(engine) = &external {
            if let Ok(mut engine) = engine.lock() {
                for p in game.handicap_points(setup.handicap) {
                    engine.play(Stone::Black, Play::Point(p));
                }
            }
        }

        let engine_name = match &external {
            Some(engine) => engine
                .lock()
                .map(|e| e.name.clone())
                .unwrap_or_else(|_| "engine".to_string()),
            None => "built-in".to_string(),
        };

        let brain = match external {
            Some(engine) => Brain::External(engine),
            None => Brain::Builtin(setup.level),
        };

        let clock = setup.main_time.map(|seconds| seconds as f64);
        let center = game.board.point(setup.board_size / 2, setup.board_size / 2);

        let mut session = Session {
            positions: vec![game.clone()],
            winrates: vec![None],
            game,
            mode: setup.mode,
            human: setup.color,
            cursor: center,
            status: String::new(),
            plays: Vec::new(),
            last: None,
            clocks: [clock, clock],
            increment: setup.increment as f64,
            hints_left: setup.hints,
            hint: None,
            show_territory: false,
            territory: None,
            undo_enabled: setup.undo,
            show_eval: setup.eval,
            thinking: false,
            hint_pending: false,
            engine_name,
            result: String::new(),
            brain,
            move_box: None,
            hint_box: None,
            eval_box: None,
            eval_due: false,
            ticked: Instant::now(),
        };

        session.request_eval();
        if session.engine_turn() {
            session.start_thinking();
        }
        session
    }

    pub fn engine_turn(&self) -> bool {
        self.mode == Mode::Engine && self.game.turn != self.human && !self.game.finished
    }

    fn human_turn(&self) -> bool {
        self.mode == Mode::Friend || self.game.turn == self.human
    }

    pub fn tick(&mut self) {
        let elapsed = self.ticked.elapsed().as_secs_f64();
        self.ticked = Instant::now();

        if let Some(receiver) = &self.hint_box {
            if let Ok(play) = receiver.try_recv() {
                self.hint_pending = false;
                self.hint_box = None;
                self.hint = Some(play);
                self.status = format!("try {}", play_name(play, self.game.size()));
            }
        }

        if let Some(receiver) = &self.eval_box {
            if let Ok((index, winrate)) = receiver.try_recv() {
                self.eval_box = None;
                if index < self.winrates.len() {
                    self.winrates[index] = Some(winrate);
                }
                if self.eval_due {
                    self.eval_due = false;
                    self.request_eval();
                }
            }
        }

        let arrived = self.move_box.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(play) = arrived {
            self.thinking = false;
            self.move_box = None;
            self.apply(self.game.turn, play);
        }

        if !self.game.finished && self.human_turn() {
            let side = side_index(self.game.turn);
            if let Some(left) = self.clocks[side].as_mut() {
                *left -= elapsed;
                if *left <= 0.0 {
                    *left = 0.0;
                    let loser = self.game.turn;
                    self.game.resign(loser);
                    self.result = format!("{} lost on time", loser.name());
                }
            }
        }
    }

    pub fn place(&mut self) -> bool {
        if !self.human_turn() || self.thinking || self.game.finished {
            return false;
        }
        let point = self.cursor;
        if !self.game.legal(point) {
            self.status = "you cannot play there".to_string();
            return false;
        }
        let color = self.game.turn;
        self.apply(color, Play::Point(point));
        true
    }

    pub fn pass_turn(&mut self) {
        if !self.human_turn() || self.thinking || self.game.finished {
            return;
        }
        let color = self.game.turn;
        self.apply(color, Play::Pass);
    }

    pub fn resign(&mut self) {
        if self.game.finished {
            return;
        }
        let loser = if self.mode == Mode::Friend {
            self.game.turn
        } else {
            self.human
        };
        self.game.resign(loser);
        self.result = format!("{} resigned", loser.name());
    }

    fn apply(&mut self, color: Stone, play: Play) {
        self.game.make(play);
        self.plays.push((color, play));
        self.positions.push(self.game.clone());
        self.winrates.push(None);
        self.last = match play {
            Play::Point(p) => Some(p),
            Play::Pass => None,
        };
        self.hint = None;
        self.status.clear();
        self.refresh_territory();

        if let Some(left) = self.clocks[side_index(color)].as_mut() {
            *left += self.increment;
        }

        if let Brain::External(engine) = &self.brain {
            if self.mode == Mode::Friend || color == self.human {
                if let Ok(mut engine) = engine.lock() {
                    engine.play(color, play);
                }
            }
        }

        if play == Play::Pass {
            self.status = format!("{} passed", color.name());
        }

        if self.game.finished {
            if self.result.is_empty() {
                let lead = self.game.lead();
                self.result = if lead > 0.0 {
                    format!("Black wins by {:.1}", lead)
                } else if lead < 0.0 {
                    format!("White wins by {:.1}", -lead)
                } else {
                    "the game is a draw".to_string()
                };
            }
            return;
        }

        self.request_eval();
        if self.engine_turn() {
            self.start_thinking();
        }
    }

    fn start_thinking(&mut self) {
        let (sender, receiver) = channel();
        self.move_box = Some(receiver);
        self.thinking = true;
        let color = self.game.turn;
        match self.brain.clone() {
            Brain::Builtin(level) => {
                let position = self.game.clone();
                std::thread::spawn(move || {
                    let report = ai::pick_move(&position, level);
                    let _ = sender.send(report.best);
                });
            }
            Brain::External(engine) => {
                std::thread::spawn(move || {
                    let play = match engine.lock() {
                        Ok(mut engine) => engine.genmove(color),
                        Err(_) => Play::Pass,
                    };
                    let _ = sender.send(play);
                });
            }
        }
    }

    pub fn ask_for_hint(&mut self) {
        if self.hint_pending || self.thinking || self.game.finished {
            return;
        }
        if self.hints_left == 0 {
            self.status = "no hints left".to_string();
            return;
        }
        if self.hints_left != u32::MAX {
            self.hints_left -= 1;
        }
        let (sender, receiver) = channel();
        self.hint_box = Some(receiver);
        self.hint_pending = true;
        self.status = "thinking about a good move".to_string();
        let color = self.game.turn;

        match self.brain.clone() {
            Brain::Builtin(_) => {
                let position = self.game.clone();
                let size = position.size();
                std::thread::spawn(move || {
                    let report = ai::search(&position, ai::budget_for(9, size));
                    let _ = sender.send(report.best);
                });
            }
            Brain::External(engine) => {
                std::thread::spawn(move || {
                    let play = match engine.lock() {
                        Ok(mut engine) => engine.suggest(color),
                        Err(_) => Play::Pass,
                    };
                    let _ = sender.send(play);
                });
            }
        }
    }

    pub fn request_eval(&mut self) {
        if !self.show_eval || self.game.finished {
            return;
        }
        if self.eval_box.is_some() {
            self.eval_due = true;
            return;
        }
        let index = self.positions.len() - 1;
        if self.winrates[index].is_some() {
            return;
        }
        let (sender, receiver) = channel();
        self.eval_box = Some(receiver);

        match self.brain.clone() {
            Brain::Builtin(_) => {
                let position = self.game.clone();
                std::thread::spawn(move || {
                    let winrate = ai::evaluate(&position, Duration::from_millis(500));
                    let _ = sender.send((index, winrate));
                });
            }
            Brain::External(engine) => {
                std::thread::spawn(move || {
                    let lead = engine.lock().ok().and_then(|mut e| e.black_lead());
                    if let Some(lead) = lead {
                        let _ = sender.send((index, winrate_from_lead(lead)));
                    }
                });
            }
        }
    }

    pub fn toggle_eval(&mut self) {
        self.show_eval = !self.show_eval;
        if self.show_eval {
            self.request_eval();
        }
    }

    pub fn toggle_territory(&mut self) {
        self.show_territory = !self.show_territory;
        self.refresh_territory();
    }

    fn refresh_territory(&mut self) {
        self.territory = if self.show_territory {
            Some(self.game.territory())
        } else {
            None
        };
    }

    pub fn undo(&mut self) {
        if !self.undo_enabled {
            self.status = "undo is switched off".to_string();
            return;
        }
        if self.thinking || self.hint_pending || self.plays.is_empty() {
            return;
        }

        let mut removed = 0;
        while self.positions.len() > 1 {
            self.plays.pop();
            self.positions.pop();
            self.winrates.pop();
            removed += 1;
            let turn = self.positions.last().map(|p| p.turn);
            if self.mode == Mode::Friend || turn == Some(self.human) {
                break;
            }
        }

        self.game = self.positions.last().cloned().unwrap();
        self.last = self.plays.last().and_then(|(_, play)| match play {
            Play::Point(p) => Some(*p),
            Play::Pass => None,
        });
        self.hint = None;
        self.result.clear();
        self.status = "took the move back".to_string();
        self.refresh_territory();

        if let Brain::External(engine) = &self.brain {
            if let Ok(mut engine) = engine.lock() {
                for _ in 0..removed {
                    engine.undo();
                }
            }
        }
    }

    pub fn move_cursor(&mut self, rows: i32, cols: i32) {
        let size = self.game.size() as i32;
        let (row, col) = self.game.board.row_col(self.cursor);
        let row = (row as i32 + rows).clamp(0, size - 1) as usize;
        let col = (col as i32 + cols).clamp(0, size - 1) as usize;
        self.cursor = self.game.board.point(row, col);
    }

    pub fn winrate(&self) -> Option<f32> {
        self.winrates.iter().rev().flatten().next().copied()
    }

    pub fn clock_text(&self, color: Stone) -> String {
        match self.clocks[side_index(color)] {
            None => "--:--".to_string(),
            Some(left) => {
                let seconds = left.max(0.0) as u64;
                format!("{:02}:{:02}", seconds / 60, seconds % 60)
            }
        }
    }

    pub fn score_text(&self) -> String {
        let lead = self.game.lead();
        if lead > 0.0 {
            format!("B+{:.1}", lead)
        } else if lead < 0.0 {
            format!("W+{:.1}", -lead)
        } else {
            "even".to_string()
        }
    }

    pub fn record(&self) -> sgf::Record {
        let mut record = sgf::Record::new(self.game.size(), self.game.komi);
        record.plays = self.plays.clone();
        record.setup_black = self.positions[0].handicap_points(self.game.handicap);
        record.result = self.result.clone();
        if self.mode == Mode::Friend {
            record.black_name = "Black".to_string();
            record.white_name = "White".to_string();
        } else if self.human == Stone::Black {
            record.white_name = self.engine_name.clone();
        } else {
            record.black_name = self.engine_name.clone();
        }
        record
    }

    pub fn save(&self) -> std::io::Result<String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("go-{}-{}.sgf", self.game.size(), stamp);
        std::fs::write(&name, sgf::write(&self.record()))?;
        Ok(name)
    }
}

pub struct Review {
    pub positions: Vec<Game>,
    pub plays: Vec<(Stone, Play)>,
    pub at: usize,
    pub analysis: Analysis,
    pub title: String,
    pub show_territory: bool,
    pub status: String,
}

impl Review {
    pub fn new(positions: Vec<Game>, plays: Vec<(Stone, Play)>, title: String) -> Review {
        let length = positions.len();
        Review {
            at: length.saturating_sub(1),
            positions,
            plays,
            analysis: Analysis::idle(length),
            title,
            show_territory: false,
            status: "press a to let the engine go through the game".to_string(),
        }
    }

    pub fn game(&self) -> &Game {
        &self.positions[self.at.min(self.positions.len() - 1)]
    }

    pub fn step(&mut self, delta: i32) {
        let last = self.positions.len() as i32 - 1;
        self.at = (self.at as i32 + delta).clamp(0, last) as usize;
    }

    pub fn analyse(&mut self) {
        if self.analysis.running() {
            self.analysis.cancel();
            self.status = "analysis stopped".to_string();
            return;
        }
        let seconds = if self.positions[0].size() >= 19 {
            1.2
        } else {
            0.6
        };
        self.analysis = Analysis::start(self.positions.clone(), seconds);
        self.status = "going through the game".to_string();
    }

    pub fn tick(&mut self) {
        self.analysis.collect();
    }

    pub fn suggestion(&self) -> Option<Play> {
        if self.at == 0 {
            return None;
        }
        self.analysis.best[self.at - 1]
    }
}

pub struct Browser {
    pub files: Vec<PathBuf>,
    pub selected: usize,
    pub message: String,
}

impl Browser {
    pub fn new() -> Browser {
        let mut browser = Browser {
            files: Vec::new(),
            selected: 0,
            message: String::new(),
        };
        browser.refresh();
        browser
    }

    pub fn refresh(&mut self) {
        let mut found: Vec<(SystemTime, PathBuf)> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("sgf") {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .unwrap_or(UNIX_EPOCH);
                found.push((modified, path));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        self.files = found.into_iter().map(|(_, path)| path).collect();
        self.selected = 0;
        self.message = if self.files.is_empty() {
            "no .sgf files in this folder".to_string()
        } else {
            String::new()
        };
    }

    pub fn move_by(&mut self, delta: i32) {
        if self.files.is_empty() {
            return;
        }
        let last = self.files.len() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }
}

pub struct App {
    pub screen: Screen,
    pub setup: Setup,
    pub session: Option<Session>,
    pub review: Option<Review>,
    pub browser: Browser,
    pub menu: usize,
    pub config: Config,
    pub theme: Theme,
    pub engine_ready: bool,
    pub quit: bool,
}

pub const MENU_ITEMS: [&str; 5] = [
    "Play a game",
    "Look at a saved game",
    "Change the look",
    "How to play",
    "Quit",
];

impl App {
    pub fn new() -> App {
        let config = Config::load();
        let engine_ready = engine_installed(&config.engine_command);
        let mut setup = Setup::from_config(&config);
        if !engine_ready {
            setup.engine = EngineChoice::Builtin;
        }
        App {
            screen: Screen::Menu,
            theme: theme_at(config.theme),
            setup,
            session: None,
            review: None,
            browser: Browser::new(),
            menu: 0,
            config,
            engine_ready,
            quit: false,
        }
    }

    pub fn engine_label(&self) -> String {
        self.config
            .engine_command
            .split_whitespace()
            .next()
            .unwrap_or("engine")
            .to_string()
    }

    pub fn start_game(&mut self) {
        self.review = None;
        self.session = Some(Session::new(&self.setup, &self.config, self.engine_ready));
        self.config.board_size = self.setup.board_size;
        self.config.level = self.setup.level;
        self.config.komi = self.setup.komi;
        self.config.show_eval = self.setup.eval;
        self.config.save();
        self.screen = Screen::Playing;
    }

    pub fn next_theme(&mut self, forward: bool) {
        let count = crate::theme::THEMES.len();
        self.config.theme = if forward {
            (self.config.theme + 1) % count
        } else {
            (self.config.theme + count - 1) % count
        };
        self.theme = theme_at(self.config.theme);
        self.config.save();
    }

    pub fn review_current_game(&mut self) {
        if let Some(session) = &self.session {
            let title = format!("{0}x{0} game", session.game.size());
            let mut review = Review::new(session.positions.clone(), session.plays.clone(), title);
            for (index, winrate) in session.winrates.iter().enumerate() {
                if let Some(value) = winrate {
                    review.analysis.winrates[index] = Some(*value);
                }
            }
            self.review = Some(review);
            self.screen = Screen::Review;
        }
    }

    pub fn open_selected_file(&mut self) {
        let path = match self.browser.files.get(self.browser.selected) {
            Some(path) => path.clone(),
            None => return,
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                self.browser.message = "could not read that file".to_string();
                return;
            }
        };
        match sgf::read(&text) {
            Some(record) => {
                let positions = sgf::positions(&record);
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("game")
                    .to_string();
                self.review = Some(Review::new(positions, record.plays.clone(), name));
                self.screen = Screen::Review;
            }
            None => self.browser.message = "that file is not a game record".to_string(),
        }
    }

    pub fn tick(&mut self) {
        match self.screen {
            Screen::Playing => {
                if let Some(session) = &mut self.session {
                    session.tick();
                }
            }
            Screen::Review => {
                if let Some(review) = &mut self.review {
                    review.tick();
                }
            }
            _ => {}
        }
    }
}

fn side_index(color: Stone) -> usize {
    if color == Stone::Black { 0 } else { 1 }
}

fn on_off(value: bool) -> String {
    if value { "on" } else { "off" }.to_string()
}

fn step(value: u8, low: u8, high: u8, forward: bool) -> u8 {
    if forward {
        (value + 1).min(high)
    } else {
        value.saturating_sub(1).max(low)
    }
}

fn cycle<T: PartialEq + Copy>(options: &[T], current: T, forward: bool) -> T {
    let count = options.len();
    let at = options.iter().position(|item| *item == current).unwrap_or(0);
    if forward {
        options[(at + 1) % count]
    } else {
        options[(at + count - 1) % count]
    }
}
