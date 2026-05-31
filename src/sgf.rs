use crate::game::{Game, Play, Stone};

pub struct Record {
    pub size: usize,
    pub komi: f32,
    pub black_name: String,
    pub white_name: String,
    pub result: String,
    pub setup_black: Vec<usize>,
    pub setup_white: Vec<usize>,
    pub plays: Vec<(Stone, Play)>,
}

impl Record {
    pub fn new(size: usize, komi: f32) -> Record {
        Record {
            size,
            komi,
            black_name: "Black".to_string(),
            white_name: "White".to_string(),
            result: String::new(),
            setup_black: Vec::new(),
            setup_white: Vec::new(),
            plays: Vec::new(),
        }
    }
}

pub fn write(record: &Record) -> String {
    let mut text = format!(
        "(;GM[1]FF[4]CA[UTF-8]AP[go-tui]SZ[{}]KM[{}]PB[{}]PW[{}]",
        record.size, record.komi, record.black_name, record.white_name
    );
    if !record.result.is_empty() {
        text.push_str(&format!("RE[{}]", record.result));
    }
    if !record.setup_black.is_empty() {
        text.push_str(&format!("HA[{}]AB", record.setup_black.len()));
        for &p in &record.setup_black {
            text.push_str(&format!("[{}]", point_to_sgf(p, record.size)));
        }
    }
    if !record.setup_white.is_empty() {
        text.push_str("AW");
        for &p in &record.setup_white {
            text.push_str(&format!("[{}]", point_to_sgf(p, record.size)));
        }
    }
    text.push('\n');

    for (index, (color, play)) in record.plays.iter().enumerate() {
        let letter = if *color == Stone::Black { 'B' } else { 'W' };
        let spot = match play {
            Play::Point(p) => point_to_sgf(*p, record.size),
            Play::Pass => String::new(),
        };
        text.push_str(&format!(";{}[{}]", letter, spot));
        if index % 12 == 11 {
            text.push('\n');
        }
    }
    text.push_str(")\n");
    text
}

pub fn read(text: &str) -> Option<Record> {
    let properties = properties(text);
    if properties.is_empty() {
        return None;
    }

    let size = properties
        .iter()
        .find(|(key, _)| key == "SZ")
        .and_then(|(_, value)| value.split(':').next()?.trim().parse().ok())
        .unwrap_or(19);
    if !(5..=25).contains(&size) {
        return None;
    }

    let komi = properties
        .iter()
        .find(|(key, _)| key == "KM")
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(6.5);

    let mut record = Record::new(size, komi);
    for (key, value) in &properties {
        match key.as_str() {
            "PB" => record.black_name = value.clone(),
            "PW" => record.white_name = value.clone(),
            "RE" => record.result = value.clone(),
            "AB" => {
                if let Some(p) = sgf_to_point(value, size) {
                    record.setup_black.push(p);
                }
            }
            "AW" => {
                if let Some(p) = sgf_to_point(value, size) {
                    record.setup_white.push(p);
                }
            }
            "B" | "W" => {
                let color = if key == "B" { Stone::Black } else { Stone::White };
                let play = match sgf_to_point(value, size) {
                    Some(p) => Play::Point(p),
                    None => Play::Pass,
                };
                record.plays.push((color, play));
            }
            _ => {}
        }
    }

    if record.plays.is_empty() && record.setup_black.is_empty() {
        return None;
    }
    Some(record)
}

pub fn positions(record: &Record) -> Vec<Game> {
    let mut game = Game::new(record.size, record.komi);
    for &p in &record.setup_black {
        game.board.put(p, Stone::Black);
    }
    for &p in &record.setup_white {
        game.board.put(p, Stone::White);
    }
    if !record.setup_black.is_empty() {
        game.handicap = record.setup_black.len() as u8;
        game.turn = Stone::White;
    }

    let mut positions = vec![game.clone()];
    for &(color, play) in &record.plays {
        game.turn = color;
        game.finished = false;
        game.make(play);
        positions.push(game.clone());
    }
    positions
}

fn properties(text: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut key = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_uppercase() {
            let mut name = String::new();
            while i < chars.len() && chars[i].is_ascii_uppercase() {
                name.push(chars[i]);
                i += 1;
            }
            key = name;
        } else if chars[i] == '[' {
            i += 1;
            let mut value = String::new();
            while i < chars.len() && chars[i] != ']' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                value.push(chars[i]);
                i += 1;
            }
            i += 1;
            if !key.is_empty() {
                found.push((key.clone(), value));
            }
        } else {
            i += 1;
        }
    }
    found
}

fn point_to_sgf(p: usize, size: usize) -> String {
    let row = p / size;
    let col = p % size;
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let col_char = letters.chars().nth(col).unwrap_or('a');
    let row_char = letters.chars().nth(row).unwrap_or('a');
    format!("{}{}", col_char, row_char)
}

fn sgf_to_point(value: &str, size: usize) -> Option<usize> {
    let letters = "abcdefghijklmnopqrstuvwxyz";
    if value.len() < 2 {
        return None;
    }
    if value == "tt" && size <= 19 {
        return None;
    }
    let mut chars = value.chars();
    let col = letters.find(chars.next()?)?;
    let row = letters.find(chars.next()?)?;
    if col >= size || row >= size {
        return None;
    }
    Some(row * size + col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_game_can_be_read_back() {
        let mut record = Record::new(9, 6.5);
        record.plays.push((Stone::Black, Play::Point(0)));
        record.plays.push((Stone::White, Play::Pass));
        record.plays.push((Stone::Black, Play::Point(40)));

        let text = write(&record);
        let back = read(&text).unwrap();
        assert_eq!(back.size, 9);
        assert_eq!(back.komi, 6.5);
        assert_eq!(back.plays.len(), 3);
        assert_eq!(back.plays[1].1, Play::Pass);
        assert_eq!(back.plays[2].1, Play::Point(40));
    }

    #[test]
    fn positions_follow_the_moves() {
        let mut record = Record::new(9, 6.5);
        record.plays.push((Stone::Black, Play::Point(0)));
        record.plays.push((Stone::White, Play::Point(1)));
        let positions = positions(&record);
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[2].board.at(1), Stone::White);
    }

    #[test]
    fn handicap_stones_are_kept() {
        let text = "(;GM[1]SZ[19]HA[2]AB[dd][pp];W[qq])";
        let record = read(text).unwrap();
        assert_eq!(record.setup_black.len(), 2);
        let positions = positions(&record);
        assert_eq!(positions[0].turn, Stone::White);
    }
}
