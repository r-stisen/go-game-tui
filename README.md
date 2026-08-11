# go-tui

A terminal UI for playing Go against a computer. Built in Rust using [ratatui](https://github.com/ratatui-org/ratatui).

![board size options: 9x9, 13x13, 19x19]

---

## What is Go?

Go is a board game where two players take turns placing stones on a grid. You capture the opponent's stones by surrounding them. The player who controls the most territory at the end wins. It is one of the oldest board games in the world and pretty fun once you get the hang of it.

---

## Features

- Play against an AI on a 9x9, 13x13 or 19x19 board
- Three difficulty levels: Easy, Medium and Hard
- Time controls (or no limit if you prefer)
- Choose to play as Black or White
- Full Go rules: captures, ko rule, suicide rule, passing, resign
- Chinese scoring with 6.5 komi for White
- Runs entirely in your terminal

---

## Requirements

You need Rust installed. If you do not have it yet, install it from [rustup.rs](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Install

Clone the repo and build it:

```bash
git clone https://github.com/rs-jensen/go-game-tui.git
cd go-tui
cargo build --release
```

The binary will be at `target/release/go-tui`.

You can also just run it without building manually:

```bash
cargo run --release
```

---

## How to play

Start the game:

```bash
./target/release/go-tui
```

### Main menu

Use the arrow keys to highlight an option and press Enter to select it.

### Setup screen

Before each game you can configure:

| Setting | Options |
|---|---|
| Board size | 9x9, 13x13, 19x19 |
| Difficulty | Easy, Medium, Hard |
| Your color | Black (goes first) or White |
| Time limit | None, 3 min, 5 min, 10 min, 20 min |

Use Up/Down to pick a row, Left/Right to change the value, and Enter to start.

### In game

| Key | Action |
|---|---|
| Arrow keys or h j k l | Move cursor |
| Enter or Space | Place stone |
| p | Pass your turn |
| r | Resign |
| q | Quit |

The cursor shows as a yellow highlight on the board. The info panel on the right shows whose turn it is, captures, clocks and controls.

---

## AI difficulty

**Easy** plays random moves. Good for learning the basics.

**Medium** uses simple heuristics. It will try to capture your stones and connect its own. A decent challenge for beginners.

**Hard** uses Monte Carlo Tree Search (MCTS) with about 1.5 seconds of thinking time per move. It plays stronger but is not perfect

---

## Scoring

The game ends when both players pass in a row. Scoring uses Chinese rules:

- Count all your stones on the board
- Count all empty points surrounded only by your stones (territory)
- White gets 6.5 komi added to their score

The player with the higher score wins.

---

## Project structure

```
src/
  main.rs   -- entry point, input handling, event loop
  game.rs   -- board, move logic, capture, scoring
  ai.rs     -- Easy / Medium / Hard AI
  app.rs    -- app state, screens, session management
  ui.rs     -- terminal rendering with ratatui
```

---

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) - terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - terminal input and control
- [rand](https://github.com/rust-random/rand) - random number generation for the AI

---

## License

MIT
