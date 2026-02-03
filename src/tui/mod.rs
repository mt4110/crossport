pub mod app;
pub mod components;
pub mod ui;

use crate::tui::app::InputMode;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new()?;
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut app::App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        KeyCode::Char('x') => app.kill_selected(),
                        KeyCode::Char('s') => app.next_sort_col(),
                        KeyCode::Char('S') => app.toggle_sort_order(),
                        KeyCode::Char('/') => app.enter_filter_mode(),
                        KeyCode::Enter => app.inspect_selected(),
                        KeyCode::Char('r') => app.restart_selected(),
                        _ => {}
                    },
                    InputMode::EditingFilter => match key.code {
                        KeyCode::Esc => {
                            app.clear_filter();
                            app.exit_filter_mode();
                        }
                        KeyCode::Enter => app.exit_filter_mode(),
                        KeyCode::Char(c) => app.append_filter(c),
                        KeyCode::Backspace => app.pop_filter(),
                        _ => {}
                    },
                    InputMode::Inspecting(_) => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.exit_inspect_mode(),
                        _ => {}
                    },
                    InputMode::ConfirmKill(_) => match key.code {
                        KeyCode::Char('y') => {
                            if let Err(_e) = app.confirm_kill() {
                                // TODO: Show error in UI? For now just print to stderr or ignore?
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.cancel_kill(),
                        _ => {}
                    },
                    InputMode::ConfirmRestart(_) => match key.code {
                        KeyCode::Char('y') => {
                            if let Err(_e) = app.confirm_restart() {
                                // TODO
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.cancel_restart(),
                        _ => {}
                    },
                }
            }
        }

        app.on_tick();
    }
}
