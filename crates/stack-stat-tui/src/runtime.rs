//! Production terminal event loop with guaranteed cleanup.

use std::{
    io::{self, Stdout},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::Utc;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use stack_stat_core::{
    config::Config,
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};
use tokio::{sync::mpsc, time::interval};

use crate::{
    app::App,
    message::{AppMessage, Effect},
    poll::{self, PollHandles},
};

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("enabling terminal raw mode")?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
            return Err(error).context("entering alternate screen");
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
                return Err(error).context("initializing TUI");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

/// Run the dashboard with independently refreshing provider tasks.
pub async fn run<G, H, S>(git: Arc<G>, github: Arc<H>, store: Arc<S>, config: Config) -> Result<()>
where
    G: GitProvider + 'static,
    H: GitHubProvider + 'static,
    S: StatusStoreProvider + 'static,
{
    let mut app = App::new(
        Arc::clone(&git),
        Arc::clone(&github),
        Arc::clone(&store),
        config.clone(),
    );
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        original_hook(info);
    }));
    let mut guard = TerminalGuard::enter()?;
    let theme = crate::theme::Theme::for_depth(crate::theme::ColorDepth::detect());
    let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();
    // The dashboard is visible before any release query begins.
    draw(&mut guard, &app, &theme)?;
    let update_tx = tx.clone();
    let _version_check = tokio::spawn(async move {
        match crate::update::check_latest(crate::update::RELEASE_REPO).await {
            Ok(Some(version)) => {
                let _ = update_tx.send(AppMessage::UpdateAvailable(version));
            }
            Err(error) => tracing::debug!(%error, "background version check failed"),
            Ok(None) => {}
        }
    });
    let pollers = poll::spawn(git, Arc::clone(&github), store, config, tx.clone());
    let mut keys = EventStream::new();
    let mut ticks = interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            event = keys.next() => match event {
                Some(Ok(Event::Key(key))) => {
                    sync_viewport(&mut app, guard.terminal.size()?.into());
                    let effects = app.handle_key(key);
                    for effect in effects {
                        if execute_effect(effect, &github, &tx, &pollers).await? { return Ok(()); }
                    }
                    sync_viewport(&mut app, guard.terminal.size()?.into());
                    draw(&mut guard, &app, &theme)?;
                }
                Some(Ok(Event::Resize(_, _))) => { sync_viewport(&mut app, guard.terminal.size()?.into()); draw(&mut guard, &app, &theme)?; },
                Some(Ok(Event::Mouse(mouse))) => {
                    let area: ratatui::layout::Rect = guard.terminal.size()?.into();
                    let target = crate::ui::hit_test(area, &app, mouse.column, mouse.row);
                    sync_viewport(&mut app, area);
                    app.handle_mouse(mouse, target);
                    draw(&mut guard, &app, &theme)?;
                }
                Some(Err(error)) => return Err(error).context("reading terminal input"),
                None => return Ok(()),
                _ => {},
            },
            message = rx.recv() => {
                let Some(message) = message else { return Ok(()); };
                let completed_mutation = matches!(message, AppMessage::MutationDone { .. });
                app.apply(message);
                pollers.set_ci_active(app.ci_active());
                if completed_mutation { pollers.refresh_all(); }
                sync_viewport(&mut app, guard.terminal.size()?.into());
                draw(&mut guard, &app, &theme)?;
            }
            _ = ticks.tick() => {
                app.tick(Utc::now());
                draw(&mut guard, &app, &theme)?;
            }
        }
    }
}
fn sync_viewport<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    app: &mut App<G, H, S>,
    area: ratatui::layout::Rect,
) {
    let detail = crate::ui::regions(area, app).detail;
    if let Some(detail) = detail {
        app.set_detail_viewport_rows(detail.height.saturating_sub(2) as usize);
    }
}

async fn execute_effect<H: GitHubProvider + 'static>(
    effect: Effect,
    github: &Arc<H>,
    tx: &mpsc::UnboundedSender<AppMessage>,
    pollers: &PollHandles,
) -> Result<bool> {
    match effect {
        Effect::Quit => Ok(true),
        Effect::Refresh(_) => {
            pollers.refresh_all();
            Ok(false)
        }
        Effect::Mutate(action) => {
            let github = Arc::clone(github);
            let tx = tx.clone();
            tokio::spawn(async move {
                let result = crate::app::execute(github.as_ref(), &action).await;
                let _ = tx.send(AppMessage::MutationDone { action, result });
            });
            Ok(false)
        }
    }
}

fn draw<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    guard: &mut TerminalGuard,
    app: &App<G, H, S>,
    theme: &crate::theme::Theme,
) -> Result<()> {
    guard
        .terminal
        .draw(|frame| crate::ui::render(frame, app, theme))
        .context("drawing terminal frame")?;
    Ok(())
}
