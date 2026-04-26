use crate::game::{Play, Stone, coord_name, parse_coord};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct GtpEngine {
    process: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    pub name: String,
    pub size: usize,
}

impl GtpEngine {
    pub fn start(command: &str, size: usize, komi: f32) -> Option<GtpEngine> {
        let mut parts = command.split_whitespace();
        let program = parts.next()?;
        let mut child = Command::new(program)
            .args(parts)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let input = BufWriter::new(child.stdin.take()?);
        let output = BufReader::new(child.stdout.take()?);
        let mut engine = GtpEngine {
            process: child,
            input,
            output,
            name: program.to_string(),
            size,
        };

        let name = engine.send("name").unwrap_or_default();
        let version = engine.send("version").unwrap_or_default();
        if !name.is_empty() {
            engine.name = if version.is_empty() {
                name
            } else {
                format!("{} {}", name, version)
            };
        }

        engine.send(&format!("boardsize {}", size))?;
        engine.send("clear_board");
        engine.send(&format!("komi {}", komi));
        Some(engine)
    }

    fn send(&mut self, command: &str) -> Option<String> {
        writeln!(self.input, "{}", command).ok()?;
        self.input.flush().ok()?;

        let mut reply = String::new();
        loop {
            let mut line = String::new();
            match self.output.read_line(&mut line) {
                Ok(0) | Err(_) => return None,
                Ok(_) => {}
            }
            if line.trim().is_empty() {
                break;
            }
            reply.push_str(line.trim());
            reply.push(' ');
        }

        let reply = reply.trim().to_string();
        if let Some(rest) = reply.strip_prefix('=') {
            Some(rest.trim().to_string())
        } else {
            None
        }
    }

    pub fn play(&mut self, color: Stone, play: Play) {
        let where_to = match play {
            Play::Point(p) => coord_name(p, self.size),
            Play::Pass => "pass".to_string(),
        };
        self.send(&format!("play {} {}", letter(color), where_to));
    }

    pub fn genmove(&mut self, color: Stone) -> Play {
        let reply = self.send(&format!("genmove {}", letter(color)));
        self.read_play(reply)
    }

    pub fn suggest(&mut self, color: Stone) -> Play {
        let reply = self.send(&format!("reg_genmove {}", letter(color)));
        self.read_play(reply)
    }

    pub fn undo(&mut self) {
        self.send("undo");
    }

    pub fn black_lead(&mut self) -> Option<f32> {
        let reply = self
            .send("estimate_score")
            .or_else(|| self.send("final_score"))?;
        parse_result(&reply)
    }

    fn read_play(&self, reply: Option<String>) -> Play {
        match reply {
            Some(text) => {
                let text = text.trim().to_uppercase();
                if text == "PASS" || text == "RESIGN" || text.is_empty() {
                    Play::Pass
                } else {
                    match parse_coord(&text, self.size) {
                        Some(p) => Play::Point(p),
                        None => Play::Pass,
                    }
                }
            }
            None => Play::Pass,
        }
    }
}

impl Drop for GtpEngine {
    fn drop(&mut self) {
        self.send("quit");
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

pub fn engine_installed(command: &str) -> bool {
    let program = match command.split_whitespace().next() {
        Some(name) => name,
        None => return false,
    };
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", program))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn letter(color: Stone) -> char {
    match color {
        Stone::White => 'W',
        _ => 'B',
    }
}

fn parse_result(reply: &str) -> Option<f32> {
    let text = reply.trim().to_uppercase();
    let text = text.replace(' ', "");
    if text.starts_with('0') || text.starts_with("DRAW") {
        return Some(0.0);
    }
    let winner = text.chars().next()?;
    let rest: String = text
        .chars()
        .skip_while(|c| *c != '+')
        .skip(1)
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let points: f32 = rest.parse().ok()?;
    match winner {
        'B' => Some(points),
        'W' => Some(-points),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_gtp_score_line() {
        assert_eq!(parse_result("B+12.5"), Some(12.5));
        assert_eq!(parse_result("W+ 3.0 (upper bound)"), Some(-3.0));
        assert_eq!(parse_result("0"), Some(0.0));
    }
}
