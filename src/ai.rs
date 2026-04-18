use crate::game::{Game, Play, Stone};
use rand::prelude::*;
use std::time::{Duration, Instant};

const EXPLORATION: f32 = 1.1;

pub struct Report {
    pub best: Play,
    pub black_winrate: f32,
    pub candidates: Vec<Candidate>,
}

pub struct Candidate {
    pub play: Play,
    pub visits: u32,
}

struct Node {
    play: Play,
    turn: Stone,
    parent: Option<usize>,
    children: Vec<usize>,
    untried: Vec<Play>,
    wins: f32,
    visits: f32,
}

impl Node {
    fn new(play: Play, game: &mut Game, parent: Option<usize>, rng: &mut SmallRng) -> Node {
        let mut untried = candidate_moves(game);
        untried.shuffle(rng);
        Node {
            play,
            turn: game.turn,
            parent,
            children: Vec::new(),
            untried,
            wins: 0.0,
            visits: 0.0,
        }
    }

    fn black_rate(&self) -> f32 {
        if self.visits < 1.0 {
            0.5
        } else {
            self.wins / self.visits
        }
    }
}

fn candidate_moves(game: &mut Game) -> Vec<Play> {
    if game.finished {
        return Vec::new();
    }
    let mut moves = Vec::new();
    for p in 0..game.board.points() {
        if !game.is_eye(p, game.turn) && game.legal(p) {
            moves.push(Play::Point(p));
        }
    }
    moves.push(Play::Pass);
    moves
}

pub fn budget_for(level: u8, size: usize) -> Duration {
    let base = 120 + 220 * level.clamp(1, 10) as u64;
    let slower = if size >= 19 { 3 } else if size >= 13 { 2 } else { 1 };
    Duration::from_millis(base * slower)
}

pub fn search(game: &Game, budget: Duration) -> Report {
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2)
        .clamp(1, 4);

    let mut trees = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| grow_tree(game, budget)));
        }
        for handle in handles {
            if let Ok(tree) = handle.join() {
                trees.push(tree);
            }
        }
    });

    let mut wins = 0.0;
    let mut visits = 0.0;
    let mut merged: Vec<Candidate> = Vec::new();
    for tree in trees {
        wins += tree.wins;
        visits += tree.visits;
        for branch in tree.branches {
            match merged.iter_mut().find(|item| item.play == branch.play) {
                Some(item) => item.visits += branch.visits,
                None => merged.push(branch),
            }
        }
    }
    merged.sort_by(|a, b| b.visits.cmp(&a.visits));

    let best = merged.first().map(|item| item.play).unwrap_or(Play::Pass);
    Report {
        best,
        black_winrate: if visits < 1.0 { 0.5 } else { wins / visits },
        candidates: merged,
    }
}

struct Tree {
    wins: f32,
    visits: f32,
    branches: Vec<Candidate>,
}

fn grow_tree(game: &Game, budget: Duration) -> Tree {
    let mut rng = SmallRng::from_rng(&mut rand::rng());
    let mut root = game.clone();
    let mut nodes = vec![Node::new(Play::Pass, &mut root, None, &mut rng)];
    let deadline = Instant::now() + budget;

    while Instant::now() < deadline {
        for _ in 0..8 {
            let mut position = game.clone();
            let mut current = 0;

            while nodes[current].untried.is_empty() && !nodes[current].children.is_empty() {
                current = select_child(&nodes, current);
                position.make(nodes[current].play);
            }

            if let Some(play) = nodes[current].untried.pop() {
                position.make(play);
                let child = Node::new(play, &mut position, Some(current), &mut rng);
                nodes.push(child);
                let index = nodes.len() - 1;
                nodes[current].children.push(index);
                current = index;
            }

            let winner = playout(&mut position, &mut rng);
            let score = if winner == Stone::Black { 1.0 } else { 0.0 };
            backup(&mut nodes, current, score);
        }
    }

    Tree {
        wins: nodes[0].wins,
        visits: nodes[0].visits,
        branches: nodes[0]
            .children
            .iter()
            .map(|&child| Candidate {
                play: nodes[child].play,
                visits: nodes[child].visits as u32,
            })
            .collect(),
    }
}

fn perspective(black_rate: f32, side: Stone) -> f32 {
    if side == Stone::Black {
        black_rate
    } else {
        1.0 - black_rate
    }
}

fn select_child(nodes: &[Node], parent: usize) -> usize {
    let logn = nodes[parent].visits.max(1.0).ln();
    let side = nodes[parent].turn;
    let mut best = nodes[parent].children[0];
    let mut best_value = f32::MIN;
    for &child in &nodes[parent].children {
        let visits = nodes[child].visits.max(1.0);
        let rate = perspective(nodes[child].black_rate(), side);
        let value = rate + EXPLORATION * (logn / visits).sqrt();
        if value > best_value {
            best_value = value;
            best = child;
        }
    }
    best
}

fn backup(nodes: &mut [Node], from: usize, black_score: f32) {
    let mut walk = Some(from);
    while let Some(index) = walk {
        nodes[index].visits += 1.0;
        nodes[index].wins += black_score;
        walk = nodes[index].parent;
    }
}

fn playout(game: &mut Game, rng: &mut SmallRng) -> Stone {
    let mut empties: Vec<usize> = (0..game.board.points()).collect();
    empties.shuffle(rng);
    let limit = game.board.points() * 2 + 20;
    let mut last = None;

    for _ in 0..limit {
        if game.finished {
            break;
        }
        let mut choice = None;
        if let Some(p) = last {
            if rng.random_bool(0.8) {
                choice = urgent_reply(game, p);
            }
        }
        if choice.is_none() {
            choice = random_move(game, &empties, rng);
        }
        match choice {
            Some(p) => {
                game.play(p);
                last = Some(p);
            }
            None => {
                game.pass();
                last = None;
            }
        }
    }

    let (black, white) = game.score();
    if black > white {
        Stone::Black
    } else {
        Stone::White
    }
}

fn random_move(game: &mut Game, empties: &[usize], rng: &mut SmallRng) -> Option<usize> {
    let start = rng.random_range(0..empties.len());
    for offset in 0..empties.len() {
        let p = empties[(start + offset) % empties.len()];
        if game.board.at(p) != Stone::Empty {
            continue;
        }
        if game.is_eye(p, game.turn) {
            continue;
        }
        if game.legal(p) {
            return Some(p);
        }
    }
    None
}

fn urgent_reply(game: &mut Game, last: usize) -> Option<usize> {
    let me = game.turn;
    if game.board.at(last) == me.other() {
        if let Some(p) = game.board.last_liberty(last) {
            if game.legal(p) {
                return Some(p);
            }
        }
    }
    for n in game.board.neighbors(last) {
        if game.board.at(n) != me {
            continue;
        }
        if let Some(p) = game.board.last_liberty(n) {
            if game.legal(p) && !game.is_eye(p, me) {
                return Some(p);
            }
        }
    }
    None
}

pub fn pick_move(game: &Game, level: u8) -> Report {
    let mut report = search(game, budget_for(level, game.size()));
    let sloppiness = match level {
        1 => 0.8,
        2 => 0.55,
        3 => 0.35,
        4 => 0.2,
        5 => 0.1,
        6 => 0.04,
        _ => 0.0,
    };
    if sloppiness > 0.0 && report.candidates.len() > 1 {
        let mut rng = rand::rng();
        if rng.random::<f32>() < sloppiness {
            let spread = (report.candidates.len() / 3).clamp(1, 12);
            let pick = rng.random_range(0..spread);
            report.best = report.candidates[pick].play;
        }
    }
    report
}

pub fn evaluate(game: &Game, budget: Duration) -> f32 {
    if game.finished {
        return if game.lead() > 0.0 { 1.0 } else { 0.0 };
    }
    search(game, budget).black_winrate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_takes_the_stone_it_can_capture() {
        let mut game = Game::new(9, 6.5);
        let white = [(0, 0), (0, 1)];
        let black = [(1, 0), (1, 1), (0, 2)];
        for &(r, c) in &white {
            game.board.put(game.board.point(r, c), Stone::White);
        }
        for &(r, c) in &black {
            game.board.put(game.board.point(r, c), Stone::Black);
        }
        game.turn = Stone::Black;
        let report = search(&game, Duration::from_millis(600));
        assert!(!report.candidates.is_empty());
        assert!(report.candidates[0].visits > 5);
    }

    #[test]
    fn the_search_beats_random_moves() {
        let mut game = Game::new(9, 6.5);
        let mut rng = SmallRng::from_rng(&mut rand::rng());
        while !game.finished && game.move_number < 200 {
            if game.turn == Stone::Black {
                let report = search(&game, Duration::from_millis(200));
                game.make(report.best);
            } else {
                let empties: Vec<usize> = (0..game.board.points()).collect();
                match random_move(&mut game, &empties, &mut rng) {
                    Some(p) => {
                        game.play(p);
                    }
                    None => game.pass(),
                }
            }
        }
        let (black, white) = game.score();
        assert!(black > white);
    }

    #[test]
    fn a_playout_always_reaches_the_end() {
        let mut rng = SmallRng::from_rng(&mut rand::rng());
        let mut game = Game::new(9, 6.5);
        let winner = playout(&mut game, &mut rng);
        assert!(winner == Stone::Black || winner == Stone::White);
        assert!(game.finished);
    }
}

