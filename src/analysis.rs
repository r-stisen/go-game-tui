use crate::ai;
use crate::game::{Game, Play, Stone};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub struct Step {
    pub index: usize,
    pub winrate: f32,
    pub best: Play,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Verdict {
    Fine,
    Inaccuracy,
    Mistake,
    Blunder,
}

impl Verdict {
    pub fn mark(self) -> &'static str {
        match self {
            Verdict::Fine => "  ",
            Verdict::Inaccuracy => "?!",
            Verdict::Mistake => " ?",
            Verdict::Blunder => "??",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Verdict::Fine => "good move",
            Verdict::Inaccuracy => "inaccuracy",
            Verdict::Mistake => "mistake",
            Verdict::Blunder => "blunder",
        }
    }
}

pub struct Analysis {
    pub winrates: Vec<Option<f32>>,
    pub best: Vec<Option<Play>>,
    pub finished: usize,
    incoming: Option<Receiver<Step>>,
    stop: Arc<AtomicBool>,
}

impl Analysis {
    pub fn idle(length: usize) -> Analysis {
        Analysis {
            winrates: vec![None; length],
            best: vec![None; length],
            finished: 0,
            incoming: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(positions: Vec<Game>, seconds_per_move: f32) -> Analysis {
        let length = positions.len();
        let (sender, receiver) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let budget = Duration::from_millis((seconds_per_move * 1000.0) as u64);

        std::thread::spawn(move || {
            for (index, position) in positions.iter().enumerate() {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let report = ai::search(position, budget);
                let step = Step {
                    index,
                    winrate: report.black_winrate,
                    best: report.best,
                };
                if sender.send(step).is_err() {
                    return;
                }
            }
        });

        Analysis {
            winrates: vec![None; length],
            best: vec![None; length],
            finished: 0,
            incoming: Some(receiver),
            stop,
        }
    }

    pub fn running(&self) -> bool {
        self.incoming.is_some() && self.finished < self.winrates.len()
    }

    pub fn collect(&mut self) {
        let mut steps = Vec::new();
        if let Some(receiver) = &self.incoming {
            while let Ok(step) = receiver.try_recv() {
                steps.push(step);
            }
        }
        for step in steps {
            if step.index < self.winrates.len() {
                self.winrates[step.index] = Some(step.winrate);
                self.best[step.index] = Some(step.best);
                self.finished += 1;
            }
        }
    }

    pub fn cancel(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.incoming = None;
    }

    pub fn verdict(&self, move_number: usize, mover: Stone) -> Option<Verdict> {
        if move_number == 0 || move_number >= self.winrates.len() {
            return None;
        }
        let before = self.winrates[move_number - 1]?;
        let after = self.winrates[move_number]?;
        let loss = if mover == Stone::Black {
            before - after
        } else {
            after - before
        };
        Some(if loss > 0.25 {
            Verdict::Blunder
        } else if loss > 0.15 {
            Verdict::Mistake
        } else if loss > 0.08 {
            Verdict::Inaccuracy
        } else {
            Verdict::Fine
        })
    }
}

impl Drop for Analysis {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn winrate_from_lead(lead: f32) -> f32 {
    1.0 / (1.0 + (-lead / 7.0).exp())
}
