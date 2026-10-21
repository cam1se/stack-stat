//! Production terminal event loop with guaranteed cleanup.

use std::{
    io::{self, Stdout},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use chrono::Utc;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent,
        KeyEventKind, KeyModifiers,
    },
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
    ui::intro::Intro,
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
    let mut intro = Some(Intro::new(theme));
    let mut intro_last_frame = Instant::now();
    let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();
    // The dashboard is visible before any release query begins.
    draw(&mut guard, &app, &theme, &mut intro)?;
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
    let mut intro_frames = interval(Duration::from_millis(33));
    loop {
        tokio::select! {
            event = keys.next() => match event {
                Some(Ok(Event::Key(key))) => {
                    if intro.is_some() {
                        match intro_key_action(key) {
                            IntroKeyAction::Quit => return Ok(()),
                            IntroKeyAction::Skip => {
                                intro.as_mut().expect("intro is present").skip();
                                sync_viewport(&mut app, guard.terminal.size()?.into());
                                draw(&mut guard, &app, &theme, &mut intro)?;
                                continue;
                            }
                            IntroKeyAction::Ignore => continue,
                        }
                    }
                    sync_viewport(&mut app, guard.terminal.size()?.into());
                    let effects = app.handle_key(key);
                    for effect in effects {
                        if execute_effect(effect, &github, &tx, &pollers).await? { return Ok(()); }
                    }
                    sync_viewport(&mut app, guard.terminal.size()?.into());
                    draw(&mut guard, &app, &theme, &mut intro)?;
                }
                Some(Ok(Event::Resize(_, _))) => {
                    sync_viewport(&mut app, guard.terminal.size()?.into());
                    advance_intro(&mut intro, &app, &mut intro_last_frame);
                    draw(&mut guard, &app, &theme, &mut intro)?;
                }
                Some(Ok(Event::Mouse(mouse))) => {
                    if intro.is_some() {
                        continue;
                    }
                    let area: ratatui::layout::Rect = guard.terminal.size()?.into();
                    let target = crate::ui::hit_test(area, &app, mouse.column, mouse.row);
                    sync_viewport(&mut app, area);
                    app.handle_mouse(mouse, target);
                    draw(&mut guard, &app, &theme, &mut intro)?;
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
                advance_intro(&mut intro, &app, &mut intro_last_frame);
                draw(&mut guard, &app, &theme, &mut intro)?;
            }
            _ = ticks.tick() => {
                app.tick(Utc::now());
                advance_intro(&mut intro, &app, &mut intro_last_frame);
                draw(&mut guard, &app, &theme, &mut intro)?;
            }
            _ = intro_frames.tick(), if intro.is_some() => {
                advance_intro(&mut intro, &app, &mut intro_last_frame);
                draw(&mut guard, &app, &theme, &mut intro)?;
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
        app.set_detail_viewport(
            detail.height.saturating_sub(2) as usize,
            detail.width.saturating_sub(2) as usize,
        );
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
        Effect::Mutate(steps) => {
            let github = Arc::clone(github);
            let tx = tx.clone();
            tokio::spawn(async move {
                let result = crate::app::execute(github.as_ref(), &steps).await;
                let _ = tx.send(AppMessage::MutationDone { steps, result });
            });
            Ok(false)
        }
    }
}

fn draw<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    guard: &mut TerminalGuard,
    app: &App<G, H, S>,
    theme: &crate::theme::Theme,
    intro: &mut Option<Intro>,
) -> Result<()> {
    guard
        .terminal
        .draw(|frame| {
            let area = frame.area();
            crate::ui::render(frame, app, theme);
            if let Some(intro) = intro.as_mut() {
                intro.apply(
                    frame.buffer_mut(),
                    area,
                    crate::ui::regions(area, app),
                    app.visible_indices().len(),
                );
            }
        })
        .context("drawing terminal frame")?;
    if intro.as_ref().is_some_and(|intro| !intro.is_active()) {
        *intro = None;
    }
    Ok(())
}

fn advance_intro<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    intro: &mut Option<Intro>,
    app: &App<G, H, S>,
    last_frame: &mut Instant,
) {
    let now = Instant::now();
    let delta = now.saturating_duration_since(*last_frame);
    *last_frame = now;
    if let Some(intro) = intro {
        intro.advance(delta, app.has_initial_github_result());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntroKeyAction {
    Quit,
    Skip,
    Ignore,
}

fn intro_key_action(key: KeyEvent) -> IntroKeyAction {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return IntroKeyAction::Ignore;
    }
    if (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        && key.modifiers == KeyModifiers::NONE)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
    {
        IntroKeyAction::Quit
    } else {
        IntroKeyAction::Skip
    }
}

#[cfg(test)]
mod tests {
    use super::{IntroKeyAction, intro_key_action};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn intro_quit_keys_exit_while_other_presses_skip() {
        assert_eq!(
            intro_key_action(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            IntroKeyAction::Quit
        );
        assert_eq!(
            intro_key_action(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            IntroKeyAction::Quit
        );
        assert_eq!(
            intro_key_action(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            IntroKeyAction::Quit
        );
        assert_eq!(
            intro_key_action(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
            IntroKeyAction::Skip
        );
    }
}
