mod app;
mod docker;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::{App, DockerAction, DockerEvent, Mode, Tab};

#[tokio::main]
async fn main() -> Result<()> {
    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channels
    let (event_tx, event_rx) = mpsc::channel::<DockerEvent>(256);
    let (action_tx, action_rx) = mpsc::channel::<DockerAction>(64);

    // Spawn Docker poller
    docker::spawn_docker_poller(event_tx, action_rx);

    // App state
    let mut app = App::new(event_rx, action_tx);

    // Render tick
    let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Main loop
    loop {
        tokio::select! {
            _ = tick_interval.tick() => {
                // Drain all pending docker events
                while let Ok(evt) = app.event_rx.try_recv() {
                    handle_docker_event(&mut app, evt);
                }

                // Poll for crossterm key events (non-blocking)
                while event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            handle_key(&mut app, key.code, key.modifiers).await;
                        }
                    }
                }

                if app.should_quit {
                    break;
                }

                // Render
                terminal.draw(|f| ui::draw(f, &mut app))?;
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_docker_event(app: &mut App, event: DockerEvent) {
    match event {
        DockerEvent::ContainersUpdated(containers) => {
            app.update_containers(containers);
        }
        DockerEvent::NetworksUpdated(networks) => {
            app.update_networks(networks);
        }
        DockerEvent::LogLine(line) => {
            app.append_log_line(line);
            // Auto-scroll to bottom if near bottom
            let total = app.log_lines.len();
            if total > 0 && app.log_scroll >= total.saturating_sub(50) {
                app.log_scroll = total.saturating_sub(1);
            }
        }
        DockerEvent::LogStreamEnded => {
            app.log_streaming = false;
        }
        DockerEvent::ActionResult { message, .. } => {
            app.status_message = Some(message);
        }
    }
}

async fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Global quit
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    match app.mode {
        Mode::Logs => handle_key_logs(app, code).await,
        Mode::Detail => handle_key_detail(app, code).await,
        Mode::Normal => handle_key_normal(app, code).await,
    }
}

async fn handle_key_normal(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.switch_tab(),
        KeyCode::BackTab => app.switch_tab(),
        KeyCode::Char('j') | KeyCode::Down => app.next_item(),
        KeyCode::Char('k') | KeyCode::Up => app.prev_item(),
        KeyCode::Enter => {
            if app.tab == Tab::Containers && app.selected_container().is_some() {
                app.mode = Mode::Detail;
            }
        }
        KeyCode::Char('l') => {
            if app.tab == Tab::Containers {
                if let Some(id) = app.selected_container_id() {
                    app.log_lines.clear();
                    app.log_scroll = 0;
                    app.log_streaming = true;
                    app.mode = Mode::Logs;
                    let _ = app.action_tx.send(DockerAction::StreamLogs { container_id: id }).await;
                }
            }
        }
        KeyCode::Char('s') => {
            if app.tab == Tab::Containers {
                if let Some(id) = app.selected_container_id() {
                    let action = match app.selected_container_state() {
                        Some("running") => DockerAction::Stop(id),
                        _ => DockerAction::Start(id),
                    };
                    let _ = app.action_tx.send(action).await;
                }
            }
        }
        KeyCode::Char('x') => {
            if app.tab == Tab::Containers {
                if let Some(id) = app.selected_container_id() {
                    let _ = app.action_tx.send(DockerAction::Kill(id)).await;
                }
            }
        }
        KeyCode::Char('r') => {
            if app.tab == Tab::Containers {
                if let Some(id) = app.selected_container_id() {
                    let _ = app.action_tx.send(DockerAction::Remove(id)).await;
                }
            }
        }
        _ => {}
    }
}

async fn handle_key_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => app.next_item(),
        KeyCode::Char('k') | KeyCode::Up => app.prev_item(),
        KeyCode::Char('l') => {
            if let Some(id) = app.selected_container_id() {
                app.log_lines.clear();
                app.log_scroll = 0;
                app.log_streaming = true;
                app.mode = Mode::Logs;
                let _ = app.action_tx.send(DockerAction::StreamLogs { container_id: id }).await;
            }
        }
        KeyCode::Char('s') => {
            if let Some(id) = app.selected_container_id() {
                let action = match app.selected_container_state() {
                    Some("running") => DockerAction::Stop(id),
                    _ => DockerAction::Start(id),
                };
                let _ = app.action_tx.send(action).await;
            }
        }
        KeyCode::Char('x') => {
            if let Some(id) = app.selected_container_id() {
                let _ = app.action_tx.send(DockerAction::Kill(id)).await;
            }
        }
        KeyCode::Char('r') => {
            if let Some(id) = app.selected_container_id() {
                let _ = app.action_tx.send(DockerAction::Remove(id)).await;
            }
        }
        _ => {}
    }
}

async fn handle_key_logs(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.log_streaming = false;
            let _ = app.action_tx.send(DockerAction::StopLogStream).await;
        }
        KeyCode::PageDown => app.log_page_down(40),
        KeyCode::PageUp => app.log_page_up(40),
        KeyCode::Char('g') => app.log_top(),
        KeyCode::Char('G') => app.log_bottom(40),
        _ => {}
    }
}
