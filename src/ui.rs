use crate::app::{
    App, Browser, MENU_ITEMS, Mode, Review, SETUP_LABELS, SETUP_ROWS, Screen, Session,
};
use crate::game::{Game, Play, Stone, play_name};
use crate::theme::{THEMES, Theme, theme_at};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

const COLUMN_LETTERS: &str = "ABCDEFGHJKLMNOPQRSTUVWXYZ";

pub fn render(frame: &mut Frame, app: &App) {
    let theme = app.theme;
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(background(theme))),
        area,
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let body = rows[0];
    let footer = rows[1];

    match app.screen {
        Screen::Menu => {
            menu_screen(frame, app, body);
            footer_line(frame, theme, footer, "up down   enter pick   tab theme   q quit");
        }
        Screen::Setup => {
            setup_screen(frame, app, body);
            footer_line(
                frame,
                theme,
                footer,
                "up down pick row   left right change   enter start   esc back",
            );
        }
        Screen::Playing => {
            playing_screen(frame, app, body);
            let session = app.session.as_ref();
            let over = session.map(|s| s.game.finished).unwrap_or(false);
            let hint = if over {
                "v review   s save   n new game   q quit"
            } else {
                "arrows move   enter play   p pass   ? hint   u undo   t territory   e eval   s save   r resign   esc menu"
            };
            footer_line(frame, theme, footer, hint);
        }
        Screen::Review => {
            review_screen(frame, app, body);
            footer_line(
                frame,
                theme,
                footer,
                "left right move   up down jump 10   a analyse   t territory   esc back",
            );
        }
        Screen::Browser => {
            browser_screen(frame, app, body);
            footer_line(frame, theme, footer, "up down pick   enter open   r refresh   esc back");
        }
        Screen::Help => {
            help_screen(frame, app, body);
            footer_line(frame, theme, footer, "any key goes back");
        }
        Screen::Themes => {
            theme_screen(frame, app, body);
            footer_line(frame, theme, footer, "left right change   enter keep it   esc back");
        }
    }
}

fn background(theme: Theme) -> Color {
    match theme.name {
        "Paper" => Color::Rgb(246, 244, 238),
        _ => Color::Rgb(18, 18, 20),
    }
}

fn framed(theme: Theme, title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(background(theme)))
}

fn inside(area: Rect, pad: u16) -> Rect {
    Rect {
        x: area.x + pad,
        y: area.y + 1,
        width: area.width.saturating_sub(pad * 2),
        height: area.height.saturating_sub(2),
    }
}

fn footer_line(frame: &mut Frame, theme: Theme, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.dim).bg(background(theme))),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn menu_screen(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme;
    let box_area = centered(area, 52, 18);
    frame.render_widget(framed(theme, "go"), box_area);

    let mut lines = vec![
        Line::from(Span::styled(
            "a game of go for the terminal",
            Style::default().fg(theme.dim),
        )),
        Line::from(""),
    ];
    for (index, item) in MENU_ITEMS.iter().enumerate() {
        let picked = index == app.menu;
        let style = if picked {
            Style::default()
                .fg(background(theme))
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };
        lines.push(Line::from(Span::styled(format!("  {}  ", item), style)));
    }
    lines.push(Line::from(""));
    let engine = if app.engine_ready {
        format!("{} found", app.engine_label())
    } else {
        format!("{} not installed, using the built-in engine", app.engine_label())
    };
    lines.push(Line::from(Span::styled(
        engine,
        Style::default().fg(theme.dim),
    )));

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        inside(box_area, 2),
    );
}

fn setup_screen(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme;
    let box_area = centered(area, 56, (SETUP_ROWS as u16) + 6);
    frame.render_widget(framed(theme, "new game"), box_area);

    let mut lines = vec![Line::from("")];
    for row in 0..SETUP_ROWS {
        let picked = row == app.setup.row;
        let enabled = app.setup.active(row);
        let value = app
            .setup
            .value(row, &app.engine_label(), app.engine_ready);
        let mut style = if enabled {
            Style::default().fg(theme.text)
        } else {
            Style::default().fg(theme.dim)
        };
        if picked {
            style = Style::default()
                .fg(background(theme))
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD);
        }
        lines.push(Line::from(Span::styled(
            format!(" {:<14}{}", SETUP_LABELS[row], value),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " press enter to start",
        Style::default().fg(theme.good),
    )));

    frame.render_widget(Paragraph::new(lines), inside(box_area, 2));
}

fn playing_screen(frame: &mut Frame, app: &App, area: Rect) {
    let session = match &app.session {
        Some(session) => session,
        None => return,
    };
    let theme = app.theme;
    let size = session.game.size();

    if area.width < board_width(size) + 26 || area.height < board_height(size) {
        too_small(frame, theme, area);
        return;
    }

    let bar_width = if session.show_eval { 4 } else { 0 };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(bar_width),
            Constraint::Min(board_width(size)),
            Constraint::Length(34),
        ])
        .split(area);

    if session.show_eval {
        eval_bar(frame, theme, columns[0], session.winrate());
    }
    let board = Board {
        game: &session.game,
        cursor: Some(session.cursor),
        last: session.last,
        mark: session.hint,
        territory: session.territory.as_ref(),
    };
    draw_board(frame, theme, columns[1], &board, "board");
    game_panel(frame, app, session, columns[2]);

    if session.game.finished {
        result_popup(frame, app, session, area);
    }
}

fn game_panel(frame: &mut Frame, app: &App, session: &Session, area: Rect) {
    let theme = app.theme;
    frame.render_widget(framed(theme, "game"), area);
    let inner = inside(area, 2);
    let game = &session.game;

    let turn_style = Style::default()
        .fg(stone_colour(theme, game.turn))
        .add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("{} ", game.turn.symbol()), turn_style),
            Span::styled(
                if session.thinking {
                    format!("{} is thinking", game.turn.name())
                } else {
                    format!("{} to play", game.turn.name())
                },
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(""),
        stat(theme, "clock black", &session.clock_text(Stone::Black)),
        stat(theme, "clock white", &session.clock_text(Stone::White)),
        stat(theme, "captures", &format!(
            "{} / {}",
            game.captured[0], game.captured[1]
        )),
        stat(theme, "score now", &session.score_text()),
    ];

    if let Some(winrate) = session.winrate() {
        lines.push(stat(
            theme,
            "black wins",
            &format!("{:.0}%", winrate * 100.0),
        ));
    }
    lines.push(Line::from(""));

    if session.mode == Mode::Engine {
        lines.push(stat(theme, "opponent", &session.engine_name));
        if session.hints_left == u32::MAX {
            lines.push(stat(theme, "hints", "unlimited"));
        } else {
            lines.push(stat(theme, "hints", &session.hints_left.to_string()));
        }
    } else {
        lines.push(stat(theme, "mode", "two players"));
    }
    lines.push(stat(theme, "move", &game.move_number.to_string()));
    lines.push(Line::from(""));

    if session.show_eval && session.winrates.iter().flatten().count() > 1 {
        lines.push(section(theme, "how the game went"));
        lines.extend(graph(theme, &session.winrates, inner.width as usize, 5, None));
        lines.push(Line::from(""));
    }

    lines.push(section(theme, "last moves"));
    for line in move_list(theme, &session.plays, game.size(), session.plays.len(), 6) {
        lines.push(line);
    }

    if !session.status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            session.status.clone(),
            Style::default().fg(theme.accent),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn review_screen(frame: &mut Frame, app: &App, area: Rect) {
    let review = match &app.review {
        Some(review) => review,
        None => return,
    };
    let theme = app.theme;
    let game = review.game();
    let size = game.size();

    if area.width < board_width(size) + 26 || area.height < board_height(size) {
        too_small(frame, theme, area);
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(board_width(size)),
            Constraint::Length(34),
        ])
        .split(area);

    let winrate = review.analysis.winrates[review.at.min(review.positions.len() - 1)];
    eval_bar(frame, theme, columns[0], winrate);

    let last = if review.at == 0 {
        None
    } else {
        match review.plays.get(review.at - 1) {
            Some((_, Play::Point(p))) => Some(*p),
            _ => None,
        }
    };
    let territory = if review.show_territory {
        Some(game.territory())
    } else {
        None
    };
    let board = Board {
        game,
        cursor: None,
        last,
        mark: review.suggestion(),
        territory: territory.as_ref(),
    };
    draw_board(frame, theme, columns[1], &board, &review.title);
    review_panel(frame, app, review, columns[2]);
}

fn review_panel(frame: &mut Frame, app: &App, review: &Review, area: Rect) {
    let theme = app.theme;
    frame.render_widget(framed(theme, "review"), area);
    let inner = inside(area, 2);
    let total = review.positions.len() - 1;
    let size = review.positions[0].size();

    let mut lines = vec![
        stat(theme, "move", &format!("{} of {}", review.at, total)),
    ];

    if review.at > 0 {
        if let Some((color, play)) = review.plays.get(review.at - 1) {
            let verdict = review.analysis.verdict(review.at, *color);
            let text = format!("{} {}", color.symbol(), play_name(*play, size));
            let mark = verdict.map(|v| v.mark().to_string()).unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<14}", "played"),
                    Style::default().fg(theme.dim),
                ),
                Span::styled(text, Style::default().fg(stone_colour(theme, *color))),
                Span::styled(
                    format!(" {}", mark),
                    Style::default().fg(theme.bad).add_modifier(Modifier::BOLD),
                ),
            ]));
            if let Some(verdict) = verdict {
                lines.push(stat(theme, "engine says", verdict.name()));
            }
        }
    }

    if let Some(winrate) = review.analysis.winrates[review.at] {
        lines.push(stat(theme, "black wins", &format!("{:.0}%", winrate * 100.0)));
    }
    if let Some(best) = review.suggestion() {
        lines.push(stat(theme, "better was", &play_name(best, size)));
    }
    lines.push(Line::from(""));

    if review.analysis.finished > 1 {
        lines.push(section(theme, "how the game went"));
        lines.extend(graph(
            theme,
            &review.analysis.winrates,
            inner.width as usize,
            6,
            Some(review.at),
        ));
        lines.push(Line::from(""));
    }

    if review.analysis.running() {
        let done = review.analysis.finished;
        lines.push(Line::from(Span::styled(
            format!("looking at move {} of {}", done, total + 1),
            Style::default().fg(theme.accent),
        )));
        lines.push(Line::from(""));
    }

    lines.push(section(theme, "moves"));
    for line in move_list(theme, &review.plays, size, review.at, 8) {
        lines.push(line);
    }

    if !review.status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            review.status.clone(),
            Style::default().fg(theme.dim),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn browser_screen(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme;
    let browser: &Browser = &app.browser;
    let box_area = centered(area, 64, 20);
    frame.render_widget(framed(theme, "saved games"), box_area);

    let mut lines = Vec::new();
    if browser.files.is_empty() {
        lines.push(Line::from(Span::styled(
            browser.message.clone(),
            Style::default().fg(theme.dim),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "games you save with s land in the folder you started from",
            Style::default().fg(theme.dim),
        )));
    } else {
        for (index, file) in browser.files.iter().take(14).enumerate() {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("game.sgf");
            let style = if index == browser.selected {
                Style::default()
                    .fg(background(theme))
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            lines.push(Line::from(Span::styled(format!(" {} ", name), style)));
        }
        if !browser.message.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                browser.message.clone(),
                Style::default().fg(theme.bad),
            )));
        }
    }

    frame.render_widget(Paragraph::new(lines), inside(box_area, 2));
}

fn theme_screen(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme;
    let box_area = centered(area, 60, 20);
    frame.render_widget(framed(theme, "look"), box_area);
    let inner = inside(box_area, 2);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let mut names = Vec::new();
    for (index, item) in THEMES.iter().enumerate() {
        let style = if index == app.config.theme {
            Style::default()
                .fg(background(theme))
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme_at(index).dim)
        };
        names.push(Span::styled(format!(" {} ", item.name), style));
    }
    frame.render_widget(
        Paragraph::new(Line::from(names)).alignment(Alignment::Center),
        rows[0],
    );

    let mut sample = Game::new(9, 6.5);
    for (row, col) in [(2, 2), (3, 4), (4, 4), (2, 6), (5, 3)] {
        sample.board.put(sample.board.point(row, col), Stone::Black);
    }
    for (row, col) in [(3, 3), (4, 5), (6, 6), (2, 4), (5, 5)] {
        sample.board.put(sample.board.point(row, col), Stone::White);
    }
    let board = Board {
        game: &sample,
        cursor: Some(sample.board.point(4, 4)),
        last: Some(sample.board.point(5, 5)),
        mark: None,
        territory: None,
    };
    draw_board(frame, theme, rows[1], &board, theme.name);
}

fn help_screen(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme;
    let box_area = centered(area, 68, 24);
    frame.render_widget(framed(theme, "how to play"), box_area);

    let text = [
        "Two players take turns putting stones on the crossings.",
        "Stones of one colour that touch each other are one group.",
        "The empty crossings next to a group are its liberties.",
        "Take a group off the board when you fill its last liberty.",
        "",
        "The game ends when both players pass.",
        "You get a point for every stone you have on the board and",
        "for every empty crossing only your stones surround.",
        "White gets komi on top because Black starts.",
        "",
        "In a game:",
        "  arrows or h j k l   move the cursor",
        "  enter or space      put a stone down",
        "  p                   pass",
        "  ?                   ask the engine for a move",
        "  u                   take a move back",
        "  t                   show who owns what",
        "  e                   show or hide the eval bar",
        "  s                   save the game to an sgf file",
        "  r                   resign",
        "",
        "Tab changes the colours at any time.",
    ];
    let lines: Vec<Line> = text
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(theme.text))))
        .collect();
    frame.render_widget(Paragraph::new(lines), inside(box_area, 2));
}

fn result_popup(frame: &mut Frame, app: &App, session: &Session, area: Rect) {
    let theme = app.theme;
    let popup = centered(area, 44, 9);
    frame.render_widget(framed(theme, "game over"), popup);

    let (black, white) = session.game.score();
    let lines = vec![
        Line::from(Span::styled(
            session.result.clone(),
            Style::default().fg(theme.good).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("black {:.1}    white {:.1}", black, white),
            Style::default().fg(theme.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "v review    s save    n new game",
            Style::default().fg(theme.dim),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        inside(popup, 2),
    );
}

struct Board<'a> {
    game: &'a Game,
    cursor: Option<usize>,
    last: Option<usize>,
    mark: Option<Play>,
    territory: Option<&'a Vec<Option<Stone>>>,
}

fn board_width(size: usize) -> u16 {
    size as u16 * 2 + 8
}

fn board_height(size: usize) -> u16 {
    size as u16 + 4
}

fn draw_board(frame: &mut Frame, theme: Theme, area: Rect, board: &Board, title: &str) {
    frame.render_widget(framed(theme, title), area);
    let size = board.game.size();
    let inner = centered(inside(area, 1), size as u16 * 2 + 6, size as u16 + 2);
    let stars: Vec<usize> = board.game.star_points();
    let mark_point = match board.mark {
        Some(Play::Point(p)) => Some(p),
        _ => None,
    };

    let letters: String = (0..size)
        .map(|col| {
            format!(
                " {}",
                COLUMN_LETTERS.chars().nth(col).unwrap_or('?')
            )
        })
        .collect();
    let header = Line::from(Span::styled(
        format!("  {}", letters),
        Style::default().fg(theme.dim),
    ));

    let mut lines = vec![header.clone()];
    for row in 0..size {
        let number = size - row;
        let mut spans = vec![Span::styled(
            format!("{:>2} ", number),
            Style::default().fg(theme.dim),
        )];
        for col in 0..size {
            let point = board.game.board.point(row, col);
            let stone = board.game.board.at(point);
            let mut style = Style::default().bg(theme.board);
            let glyph = if stone != Stone::Empty {
                style = style.fg(stone_colour(theme, stone)).add_modifier(Modifier::BOLD);
                if board.last == Some(point) {
                    if stone == Stone::Black { "◉" } else { "◎" }
                } else {
                    stone.symbol()
                }
            } else if mark_point == Some(point) {
                style = style.fg(theme.good).add_modifier(Modifier::BOLD);
                "◇"
            } else if let Some(owner) = board.territory.and_then(|map| map[point]) {
                style = style.fg(stone_colour(theme, owner));
                if owner == Stone::Black { "▪" } else { "▫" }
            } else if stars.contains(&point) {
                style = style.fg(theme.star);
                "╋"
            } else {
                style = style.fg(theme.line);
                grid_glyph(row, col, size)
            };

            if board.cursor == Some(point) {
                style = style.bg(theme.cursor).fg(background(theme));
            }
            spans.push(Span::styled(glyph.to_string(), style));

            let joint = if col + 1 < size { "─" } else { " " };
            spans.push(Span::styled(
                joint,
                Style::default().fg(theme.line).bg(theme.board),
            ));
        }
        spans.push(Span::styled(
            format!("{:>2}", number),
            Style::default().fg(theme.dim),
        ));
        lines.push(Line::from(spans));
    }
    lines.push(header);

    frame.render_widget(Paragraph::new(lines), inner);
}

fn grid_glyph(row: usize, col: usize, size: usize) -> &'static str {
    let top = row == 0;
    let bottom = row + 1 == size;
    let left = col == 0;
    let right = col + 1 == size;
    match (top, bottom, left, right) {
        (true, _, true, _) => "┌",
        (true, _, _, true) => "┐",
        (_, true, true, _) => "└",
        (_, true, _, true) => "┘",
        (true, _, _, _) => "┬",
        (_, true, _, _) => "┴",
        (_, _, true, _) => "├",
        (_, _, _, true) => "┤",
        _ => "┼",
    }
}

fn eval_bar(frame: &mut Frame, theme: Theme, area: Rect, winrate: Option<f32>) {
    let rate = winrate.unwrap_or(0.5);
    let height = area.height.saturating_sub(2) as usize;
    if height == 0 {
        return;
    }
    let black_rows = (rate * height as f32).round() as usize;

    let mut lines = vec![Line::from(Span::styled(
        format!("{:>3}", (100.0 - rate * 100.0).round() as u32),
        Style::default().fg(theme.dim),
    ))];
    for row in 0..height {
        let from_bottom = height - row;
        let filled = from_bottom <= black_rows;
        let colour = if filled { theme.black } else { theme.white };
        lines.push(Line::from(Span::styled(
            "███",
            Style::default().fg(colour).bg(theme.board),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!("{:>3}", (rate * 100.0).round() as u32),
        Style::default().fg(theme.dim),
    )));

    frame.render_widget(Paragraph::new(lines), area);
}

fn graph(
    theme: Theme,
    winrates: &[Option<f32>],
    width: usize,
    height: usize,
    at: Option<usize>,
) -> Vec<Line<'static>> {
    let width = width.min(30).max(4);
    let mut samples = Vec::new();
    let mut markers = Vec::new();
    let mut carried = 0.5;
    for column in 0..width {
        let index = column * winrates.len() / width;
        if let Some(value) = winrates.get(index).copied().flatten() {
            carried = value;
        }
        samples.push(carried);
        markers.push(at.map(|position| index >= position).unwrap_or(false));
    }

    let mut lines = Vec::new();
    for row in 0..height {
        let mut spans = Vec::new();
        for (column, rate) in samples.iter().enumerate() {
            let black_rows = (rate * height as f32).round() as usize;
            let from_bottom = height - row;
            let colour = if from_bottom <= black_rows {
                theme.black
            } else {
                theme.white
            };
            let marked = markers[column] && !markers[column.saturating_sub(1)];
            let style = if marked && column > 0 {
                Style::default().fg(theme.marker).bg(theme.board)
            } else {
                Style::default().fg(colour).bg(theme.board)
            };
            spans.push(Span::styled(if marked && column > 0 { "│" } else { "█" }, style));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn move_list(
    theme: Theme,
    plays: &[(Stone, Play)],
    size: usize,
    around: usize,
    count: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let end = around.min(plays.len());
    let start = end.saturating_sub(count);
    for (offset, (color, play)) in plays[start..end].iter().enumerate() {
        let number = start + offset + 1;
        let style = if number == around {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "{:>3}. {} {}",
                number,
                color.symbol(),
                play_name(*play, size)
            ),
            style,
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no moves yet",
            Style::default().fg(theme.dim),
        )));
    }
    lines
}

fn stat(theme: Theme, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<14}", label.to_string()),
            Style::default().fg(theme.dim),
        ),
        Span::styled(value.to_string(), Style::default().fg(theme.text)),
    ])
}

fn section(theme: Theme, label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_string(),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ))
}

fn stone_colour(theme: Theme, stone: Stone) -> Color {
    match stone {
        Stone::Black => theme.black,
        Stone::White => theme.white,
        Stone::Empty => theme.line,
    }
}

fn too_small(frame: &mut Frame, theme: Theme, area: Rect) {
    frame.render_widget(
        Paragraph::new("the window is too small for this board")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.bad)),
        centered(area, 40, 1),
    );
}
