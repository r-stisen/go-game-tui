use std::fs;
use std::path::PathBuf;

pub struct Config {
    pub theme: usize,
    pub board_size: usize,
    pub level: u8,
    pub komi: f32,
    pub engine_command: String,
    pub show_eval: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            theme: 0,
            board_size: 19,
            level: 8,
            komi: 6.5,
            engine_command: "gnugo --mode gtp".to_string(),
            show_eval: true,
        }
    }
}

impl Config {
    pub fn load() -> Config {
        let mut config = Config::default();
        let text = match path().and_then(|p| fs::read_to_string(p).ok()) {
            Some(text) => text,
            None => return config,
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = match line.split_once('=') {
                Some(pair) => (pair.0.trim(), pair.1.trim()),
                None => continue,
            };
            match key {
                "theme" => config.theme = value.parse().unwrap_or(config.theme),
                "board_size" => config.board_size = value.parse().unwrap_or(config.board_size),
                "level" => config.level = value.parse().unwrap_or(config.level),
                "komi" => config.komi = value.parse().unwrap_or(config.komi),
                "engine_command" => config.engine_command = value.to_string(),
                "show_eval" => config.show_eval = value == "true",
                _ => {}
            }
        }
        config
    }

    pub fn save(&self) {
        let file = match path() {
            Some(file) => file,
            None => return,
        };
        if let Some(folder) = file.parent() {
            let _ = fs::create_dir_all(folder);
        }
        let text = format!(
            "theme = {}\nboard_size = {}\nlevel = {}\nkomi = {}\nengine_command = {}\nshow_eval = {}\n",
            self.theme, self.board_size, self.level, self.komi, self.engine_command, self.show_eval
        );
        let _ = fs::write(file, text);
    }
}

fn path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/go-tui/config"))
}
