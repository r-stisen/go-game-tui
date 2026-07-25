mod ai;
mod analysis;
mod app;
mod config;
mod game;
mod gtp;
mod sgf;
mod theme;
mod ui;

use app::{App, Screen, SETUP_ROWS, MENU_ITEMS};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let outcome = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    outcome
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    while !app.quit {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break;
                    }
                    handle(&mut app, key.code);
                }
            }
        }
        app.tick();
    }
    Ok(())
}

fn handle(app: &mut App, key: KeyCode) {
    if key == KeyCode::Tab {
        app.next_theme(true);
        return;
    }
    match app.screen {
        Screen::Menu => menu_keys(app, key),
        Screen::Setup => setup_keys(app, key),
        Screen::Playing => playing_keys(app, key),
        Screen::Review => review_keys(app, key),
        Screen::Browser => browser_keys(app, key),
        Screen::Themes => theme_keys(app, key),
        Screen::Help => app.screen = Screen::Menu,
    }
}

fn menu_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => app.menu = app.menu.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => {
            app.menu = (app.menu + 1).min(MENU_ITEMS.len() - 1)
        }
        KeyCode::Enter => match app.menu {
            0 => app.screen = Screen::Setup,
            1 => {
                app.browser.refresh();
                app.screen = Screen::Browser;
            }
            2 => app.screen = Screen::Themes,
            3 => app.screen = Screen::Help,
            _ => app.quit = true,
        },
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn setup_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => app.setup.row = app.setup.row.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => {
            app.setup.row = (app.setup.row + 1).min(SETUP_ROWS - 1)
        }
        KeyCode::Left | KeyCode::Char('h') => app.setup.change(false),
        KeyCode::Right | KeyCode::Char('l') => app.setup.change(true),
        KeyCode::Enter => app.start_game(),
        KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn playing_keys(app: &mut App, key: KeyCode) {
    let over = app
        .session
        .as_ref()
        .map(|session| session.game.finished)
        .unwrap_or(false);

    if over {
        match key {
            KeyCode::Char('v') => app.review_current_game(),
            KeyCode::Char('s') => save_current(app),
            KeyCode::Char('n') => app.screen = Screen::Setup,
            KeyCode::Esc => app.screen = Screen::Menu,
            KeyCode::Char('q') => app.quit = true,
            _ => {}
        }
        return;
    }

    let session = match &mut app.session {
        Some(session) => session,
        None => return,
    };

    match key {
        KeyCode::Up | KeyCode::Char('k') => session.move_cursor(-1, 0),
        KeyCode::Down | KeyCode::Char('j') => session.move_cursor(1, 0),
        KeyCode::Left | KeyCode::Char('h') => session.move_cursor(0, -1),
        KeyCode::Right | KeyCode::Char('l') => session.move_cursor(0, 1),
        KeyCode::Enter | KeyCode::Char(' ') => {
            session.place();
        }
        KeyCode::Char('p') => session.pass_turn(),
        KeyCode::Char('u') => session.undo(),
        KeyCode::Char('?') | KeyCode::Char('i') => session.ask_for_hint(),
        KeyCode::Char('t') => session.toggle_territory(),
        KeyCode::Char('e') => session.toggle_eval(),
        KeyCode::Char('r') => session.resign(),
        KeyCode::Char('s') => save_current(app),
        KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn review_keys(app: &mut App, key: KeyCode) {
    let review = match &mut app.review {
        Some(review) => review,
        None => return,
    };
    match key {
        KeyCode::Left | KeyCode::Char('h') => review.step(-1),
        KeyCode::Right | KeyCode::Char('l') => review.step(1),
        KeyCode::Down | KeyCode::Char('j') => review.step(-10),
        KeyCode::Up | KeyCode::Char('k') => review.step(10),
        KeyCode::Home | KeyCode::Char('0') => review.at = 0,
        KeyCode::End | KeyCode::Char('$') => review.at = review.positions.len() - 1,
        KeyCode::Char('a') => review.analyse(),
        KeyCode::Char('t') => review.show_territory = !review.show_territory,
        KeyCode::Esc => {
            review.analysis.cancel();
            app.screen = if app.session.is_some() {
                Screen::Playing
            } else {
                Screen::Menu
            };
        }
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn browser_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k') => app.browser.move_by(-1),
        KeyCode::Down | KeyCode::Char('j') => app.browser.move_by(1),
        KeyCode::Enter => app.open_selected_file(),
        KeyCode::Char('r') => app.browser.refresh(),
        KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn theme_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Left | KeyCode::Char('h') => app.next_theme(false),
        KeyCode::Right | KeyCode::Char('l') => app.next_theme(true),
        KeyCode::Enter | KeyCode::Esc => app.screen = Screen::Menu,
        KeyCode::Char('q') => app.quit = true,
        _ => {}
    }
}

fn save_current(app: &mut App) {
    if let Some(session) = &mut app.session {
        session.status = match session.save() {
            Ok(name) => format!("saved as {}", name),
            Err(problem) => format!("could not save: {}", problem),
        };
    }
}

