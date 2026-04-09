#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stone {
    Empty,
    Black,
    White,
}

impl Stone {
    pub fn other(self) -> Stone {
        match self {
            Stone::Black => Stone::White,
            Stone::White => Stone::Black,
            Stone::Empty => Stone::Empty,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stone::Black => "Black",
            Stone::White => "White",
            Stone::Empty => "Nobody",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Stone::Black => "●",
            Stone::White => "○",
            Stone::Empty => " ",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Play {
    Point(usize),
    Pass,
}

pub struct Neighbors {
    points: [usize; 4],
    count: usize,
    at: usize,
}

impl Iterator for Neighbors {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.at < self.count {
            self.at += 1;
            Some(self.points[self.at - 1])
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct Board {
    pub size: usize,
    cells: Vec<Stone>,
    seen: Vec<u32>,
    stamp: u32,
    stack: Vec<usize>,
}

impl Board {
    pub fn new(size: usize) -> Board {
        Board {
            size,
            cells: vec![Stone::Empty; size * size],
            seen: vec![0; size * size],
            stamp: 0,
            stack: Vec::with_capacity(size * size),
        }
    }

    pub fn points(&self) -> usize {
        self.cells.len()
    }

    pub fn point(&self, row: usize, col: usize) -> usize {
        row * self.size + col
    }

    pub fn row_col(&self, p: usize) -> (usize, usize) {
        (p / self.size, p % self.size)
    }

    pub fn at(&self, p: usize) -> Stone {
        self.cells[p]
    }

    pub fn put(&mut self, p: usize, stone: Stone) {
        self.cells[p] = stone;
    }

    pub fn neighbors(&self, p: usize) -> Neighbors {
        let (row, col) = self.row_col(p);
        let mut points = [0usize; 4];
        let mut count = 0;
        if row > 0 {
            points[count] = p - self.size;
            count += 1;
        }
        if row + 1 < self.size {
            points[count] = p + self.size;
            count += 1;
        }
        if col > 0 {
            points[count] = p - 1;
            count += 1;
        }
        if col + 1 < self.size {
            points[count] = p + 1;
            count += 1;
        }
        Neighbors {
            points,
            count,
            at: 0,
        }
    }

    pub fn diagonals(&self, p: usize) -> Neighbors {
        let (row, col) = self.row_col(p);
        let mut points = [0usize; 4];
        let mut count = 0;
        if row > 0 && col > 0 {
            points[count] = p - self.size - 1;
            count += 1;
        }
        if row > 0 && col + 1 < self.size {
            points[count] = p - self.size + 1;
            count += 1;
        }
        if row + 1 < self.size && col > 0 {
            points[count] = p + self.size - 1;
            count += 1;
        }
        if row + 1 < self.size && col + 1 < self.size {
            points[count] = p + self.size + 1;
            count += 1;
        }
        Neighbors {
            points,
            count,
            at: 0,
        }
    }

    pub fn on_edge(&self, p: usize) -> bool {
        let (row, col) = self.row_col(p);
        row == 0 || col == 0 || row + 1 == self.size || col + 1 == self.size
    }

    fn start_walk(&mut self, start: usize) -> u32 {
        self.stamp += 1;
        if self.stamp == u32::MAX {
            for slot in self.seen.iter_mut() {
                *slot = 0;
            }
            self.stamp = 1;
        }
        self.stack.clear();
        self.stack.push(start);
        self.seen[start] = self.stamp;
        self.stamp
    }

    pub fn liberties(&mut self, start: usize) -> usize {
        let color = self.cells[start];
        let stamp = self.start_walk(start);
        let mut liberties = 0;
        while let Some(p) = self.stack.pop() {
            for n in self.neighbors(p) {
                if self.seen[n] == stamp {
                    continue;
                }
                self.seen[n] = stamp;
                if self.cells[n] == Stone::Empty {
                    liberties += 1;
                } else if self.cells[n] == color {
                    self.stack.push(n);
                }
            }
        }
        liberties
    }

    pub fn last_liberty(&mut self, start: usize) -> Option<usize> {
        let color = self.cells[start];
        let stamp = self.start_walk(start);
        let mut found = None;
        while let Some(p) = self.stack.pop() {
            for n in self.neighbors(p) {
                if self.seen[n] == stamp {
                    continue;
                }
                self.seen[n] = stamp;
                if self.cells[n] == Stone::Empty {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(n);
                } else if self.cells[n] == color {
                    self.stack.push(n);
                }
            }
        }
        found
    }

    pub fn alone(&self, p: usize) -> bool {
        let color = self.cells[p];
        !self.neighbors(p).any(|n| self.cells[n] == color)
    }

    fn take_group(&mut self, start: usize) -> (usize, usize) {
        let color = self.cells[start];
        let stamp = self.start_walk(start);
        let mut taken = 0;
        let mut last = start;
        while let Some(p) = self.stack.pop() {
            self.cells[p] = Stone::Empty;
            taken += 1;
            last = p;
            for n in self.neighbors(p) {
                if self.seen[n] != stamp && self.cells[n] == color {
                    self.seen[n] = stamp;
                    self.stack.push(n);
                }
            }
        }
        (taken, last)
    }
}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub turn: Stone,
    pub captured: [usize; 2],
    pub passes: usize,
    pub finished: bool,
    pub winner: Option<Stone>,
    pub komi: f32,
    pub ko: Option<usize>,
    pub move_number: usize,
    pub handicap: u8,
}

impl Game {
    pub fn new(size: usize, komi: f32) -> Game {
        Game {
            board: Board::new(size),
            turn: Stone::Black,
            captured: [0, 0],
            passes: 0,
            finished: false,
            winner: None,
            komi,
            ko: None,
            move_number: 0,
            handicap: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.board.size
    }

    pub fn legal(&mut self, p: usize) -> bool {
        if self.finished || self.board.at(p) != Stone::Empty || self.ko == Some(p) {
            return false;
        }
        let me = self.turn;
        for n in self.board.neighbors(p) {
            let stone = self.board.at(n);
            if stone == Stone::Empty {
                return true;
            }
            if stone == me {
                if self.board.liberties(n) > 1 {
                    return true;
                }
            } else if self.board.liberties(n) == 1 {
                return true;
            }
        }
        false
    }

    pub fn play(&mut self, p: usize) -> bool {
        if !self.legal(p) {
            return false;
        }
        let me = self.turn;
        let enemy = me.other();
        self.board.put(p, me);

        let mut taken = 0;
        let mut last_taken = 0;
        for n in self.board.neighbors(p) {
            if self.board.at(n) == enemy && self.board.liberties(n) == 0 {
                let (count, last) = self.board.take_group(n);
                taken += count;
                last_taken = last;
            }
        }

        let single = self.board.alone(p);
        let liberties = self.board.liberties(p);
        self.ko = if taken == 1 && single && liberties == 1 {
            Some(last_taken)
        } else {
            None
        };

        if me == Stone::Black {
            self.captured[0] += taken;
        } else {
            self.captured[1] += taken;
        }
        self.passes = 0;
        self.move_number += 1;
        self.turn = enemy;
        true
    }

    pub fn pass(&mut self) {
        if self.finished {
            return;
        }
        self.passes += 1;
        self.move_number += 1;
        self.ko = None;
        self.turn = self.turn.other();
        if self.passes >= 2 {
            self.finished = true;
        }
    }

    pub fn make(&mut self, play: Play) -> bool {
        match play {
            Play::Point(p) => self.play(p),
            Play::Pass => {
                self.pass();
                true
            }
        }
    }

    pub fn resign(&mut self, loser: Stone) {
        self.finished = true;
        self.winner = Some(loser.other());
    }

    pub fn is_eye(&self, p: usize, color: Stone) -> bool {
        if self.board.at(p) != Stone::Empty {
            return false;
        }
        for n in self.board.neighbors(p) {
            if self.board.at(n) != color {
                return false;
            }
        }
        let mut enemy_corners = 0;
        for d in self.board.diagonals(p) {
            if self.board.at(d) == color.other() {
                enemy_corners += 1;
            }
        }
        if self.board.on_edge(p) {
            enemy_corners == 0
        } else {
            enemy_corners <= 1
        }
    }

    pub fn score(&self) -> (f32, f32) {
        let mut black = 0.0;
        let mut white = self.komi;
        let owners = self.territory();
        for p in 0..self.board.points() {
            match self.board.at(p) {
                Stone::Black => black += 1.0,
                Stone::White => white += 1.0,
                Stone::Empty => match owners[p] {
                    Some(Stone::Black) => black += 1.0,
                    Some(Stone::White) => white += 1.0,
                    _ => {}
                },
            }
        }
        (black, white)
    }

    pub fn lead(&self) -> f32 {
        let (black, white) = self.score();
        black - white
    }

    pub fn territory(&self) -> Vec<Option<Stone>> {
        let count = self.board.points();
        let mut owners = vec![None; count];
        let mut seen = vec![false; count];

        for start in 0..count {
            if self.board.at(start) != Stone::Empty || seen[start] {
                continue;
            }
            let mut region = Vec::new();
            let mut touches_black = false;
            let mut touches_white = false;
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(p) = stack.pop() {
                region.push(p);
                for n in self.board.neighbors(p) {
                    match self.board.at(n) {
                        Stone::Black => touches_black = true,
                        Stone::White => touches_white = true,
                        Stone::Empty => {
                            if !seen[n] {
                                seen[n] = true;
                                stack.push(n);
                            }
                        }
                    }
                }
            }
            let owner = match (touches_black, touches_white) {
                (true, false) => Some(Stone::Black),
                (false, true) => Some(Stone::White),
                _ => None,
            };
            for p in region {
                owners[p] = owner;
            }
        }
        owners
    }

    pub fn place_handicap(&mut self, stones: u8) {
        for p in self.handicap_points(stones) {
            self.board.put(p, Stone::Black);
        }
        if stones > 0 {
            self.handicap = stones;
            self.turn = Stone::White;
        }
    }

    pub fn handicap_points(&self, stones: u8) -> Vec<usize> {
        let size = self.board.size;
        if size < 9 || stones < 2 {
            return Vec::new();
        }
        let edge = if size > 9 { 3 } else { 2 };
        let far = size - 1 - edge;
        let mid = size / 2;
        let corners = [(far, edge), (edge, far), (edge, edge), (far, far)];
        let sides = [(mid, edge), (mid, far), (edge, mid), (far, mid)];

        let count = stones.min(9) as usize;
        let mut spots = corners[..count.min(4)].to_vec();
        if count > 4 {
            let side_count = if count % 2 == 1 { count - 5 } else { count - 4 };
            spots.extend(sides.iter().take(side_count));
            if count % 2 == 1 {
                spots.push((mid, mid));
            }
        }
        spots
            .into_iter()
            .map(|(r, c)| self.board.point(r, c))
            .collect()
    }

    pub fn star_points(&self) -> Vec<usize> {
        let size = self.board.size;
        if size < 9 {
            return Vec::new();
        }
        let edge = if size > 9 { 3 } else { 2 };
        let far = size - 1 - edge;
        let mid = size / 2;
        let mut spots = vec![(edge, edge), (edge, far), (far, edge), (far, far)];
        if size % 2 == 1 {
            spots.push((mid, mid));
        }
        if size >= 19 {
            spots.extend([(edge, mid), (mid, edge), (mid, far), (far, mid)]);
        }
        spots
            .into_iter()
            .map(|(r, c)| self.board.point(r, c))
            .collect()
    }
}

pub fn coord_name(p: usize, size: usize) -> String {
    let letters = "ABCDEFGHJKLMNOPQRSTUVWXYZ";
    let row = p / size;
    let col = p % size;
    let letter = letters.chars().nth(col).unwrap_or('?');
    format!("{}{}", letter, size - row)
}

pub fn play_name(play: Play, size: usize) -> String {
    match play {
        Play::Point(p) => coord_name(p, size),
        Play::Pass => "pass".to_string(),
    }
}

pub fn parse_coord(text: &str, size: usize) -> Option<usize> {
    let letters = "ABCDEFGHJKLMNOPQRSTUVWXYZ";
    let text = text.trim().to_uppercase();
    let mut chars = text.chars();
    let letter = chars.next()?;
    let col = letters.find(letter)?;
    let number: usize = chars.as_str().parse().ok()?;
    if col >= size || number == 0 || number > size {
        return None;
    }
    Some((size - number) * size + col)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(game: &Game, row: usize, col: usize) -> usize {
        game.board.point(row, col)
    }

    #[test]
    fn capture_removes_the_group() {
        let mut game = Game::new(9, 6.5);
        game.play(at(&game, 0, 1));
        game.play(at(&game, 0, 0));
        game.play(at(&game, 1, 0));
        assert_eq!(game.board.at(at(&game, 0, 0)), Stone::Empty);
        assert_eq!(game.captured[0], 1);
    }

    #[test]
    fn suicide_is_not_allowed() {
        let mut game = Game::new(9, 6.5);
        game.play(at(&game, 0, 1));
        game.pass();
        game.play(at(&game, 1, 0));
        assert_eq!(game.turn, Stone::White);
        assert!(!game.legal(at(&game, 0, 0)));
    }

    #[test]
    fn filling_the_last_liberty_takes_the_whole_group() {
        let mut game = Game::new(9, 6.5);
        for (row, col) in [(0, 0), (0, 1)] {
            game.board.put(at(&game, row, col), Stone::White);
        }
        for (row, col) in [(1, 0), (1, 1)] {
            game.board.put(at(&game, row, col), Stone::Black);
        }
        game.turn = Stone::Black;
        assert!(game.play(at(&game, 0, 2)));
        assert_eq!(game.board.at(at(&game, 0, 0)), Stone::Empty);
        assert_eq!(game.board.at(at(&game, 0, 1)), Stone::Empty);
        assert_eq!(game.captured[0], 2);
    }

    #[test]
    fn ko_blocks_the_immediate_recapture() {
        let mut game = Game::new(9, 6.5);
        let black = [(0, 1), (1, 0), (2, 1)];
        let white = [(0, 2), (1, 3), (2, 2), (1, 1)];
        for &(row, col) in &black {
            game.board.put(at(&game, row, col), Stone::Black);
        }
        for &(row, col) in &white {
            game.board.put(at(&game, row, col), Stone::White);
        }
        game.turn = Stone::Black;
        assert!(game.play(at(&game, 1, 2)));
        assert_eq!(game.board.at(at(&game, 1, 1)), Stone::Empty);
        assert!(!game.legal(at(&game, 1, 1)));
    }

    #[test]
    fn territory_goes_to_the_surrounding_colour() {
        let mut game = Game::new(9, 0.0);
        for col in 0..9 {
            game.board.put(game.board.point(4, col), Stone::Black);
        }
        let owners = game.territory();
        assert_eq!(owners[game.board.point(0, 0)], Some(Stone::Black));
        assert_eq!(owners[game.board.point(8, 0)], Some(Stone::Black));
    }

    #[test]
    fn two_passes_end_the_game() {
        let mut game = Game::new(9, 6.5);
        game.pass();
        game.pass();
        assert!(game.finished);
    }

    #[test]
    fn coordinates_skip_the_letter_i() {
        assert_eq!(coord_name(0, 19), "A19");
        assert_eq!(coord_name(8, 19), "J19");
        assert_eq!(parse_coord("J19", 19), Some(8));
        assert_eq!(parse_coord("I19", 19), None);
    }
}
