//! Event loop, effect runner, terminal lifecycle.

use std::io;
use std::time::{Duration, Instant};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;

use super::effect::Effect;
use super::keys;
use super::msg::Msg;
use super::state::{Modal, State};
use super::update::update;
use super::view;

pub type Term = Terminal<CrosstermBackend<io::Stdout>>;

pub fn enter_tui() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    // No EnableMouseCapture: we don't consume mouse events, and capturing
    // them would block the terminal's native text selection — which is how
    // the user copies the cache path out of the details pane.
    execute!(out, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(out))
}

pub fn leave_tui(term: &mut Term) -> io::Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
    let _ = term.show_cursor();
    Ok(())
}

struct EffectRunner {
    msg_tx: std::sync::mpsc::Sender<Msg>,
}

impl EffectRunner {
    fn run(&self, eff: Effect) {
        let tx = self.msg_tx.clone();
        match eff {
            Effect::SpawnScan {
                parent_label,
                parent_path,
            } => {
                std::thread::spawn(move || {
                    // Same throttled folder-count stream as LoadSeeds so the
                    // spinner reads identically for drill-in scans.
                    const PROGRESS_EVERY: usize = 200;
                    let mut total = 0usize;
                    let progress_tx = tx.clone();
                    let children = crate::caches::scan::enumerate_seed_with_progress(
                        &parent_path,
                        &mut || {
                            total += 1;
                            if total.is_multiple_of(PROGRESS_EVERY) {
                                let _ = progress_tx.send(Msg::ScanProgress { folders: total });
                            }
                        },
                    );
                    let _ = tx.send(Msg::ScanProgress { folders: total });
                    let _ = tx.send(Msg::ScanCompleted {
                        parent_label,
                        parent_path,
                        children,
                    });
                });
            }
            Effect::SpawnRefresh { path } => {
                std::thread::spawn(move || {
                    let cache = crate::caches::scan::stat_dir(&path);
                    let _ = tx.send(Msg::RefreshCompleted { path, cache });
                });
            }
            Effect::SpawnDelete { items, dry_run } => {
                std::thread::spawn(move || {
                    use crate::cleaner::{Clean, DoCleanUp, DryRunCleaner, ProperCleaner};
                    let cleaner: Box<dyn DoCleanUp> = if dry_run {
                        Box::new(DryRunCleaner)
                    } else {
                        Box::new(ProperCleaner)
                    };
                    let mut freed = 0u64;
                    let mut deleted_count = 0usize;
                    let mut failed_count = 0usize;
                    let mut deleted_indices: Vec<usize> = Vec::new();
                    for (idx, path, size) in &items {
                        match cleaner.do_cleanup(path) {
                            Ok(Clean::Cleaned) => {
                                freed += *size;
                                deleted_count += 1;
                                deleted_indices.push(*idx);
                            }
                            Ok(Clean::NotCleaned) => {
                                freed += *size;
                                deleted_count += 1;
                            }
                            Err(_) => {
                                failed_count += 1;
                            }
                        }
                    }
                    let _ = tx.send(Msg::DeleteCompleted {
                        freed,
                        deleted_count,
                        failed_count,
                        deleted_indices,
                    });
                });
            }
            Effect::EmitAfter { dur, msg } => {
                std::thread::spawn(move || {
                    std::thread::sleep(dur);
                    let _ = tx.send(msg);
                });
            }
            Effect::LoadSeeds { seeds } => {
                std::thread::spawn(move || {
                    // Stream folder counts back so the spinner shows progress
                    // instead of just elapsed time.  Throttle to roughly once
                    // per 200 directories so we don't spam the channel.
                    const PROGRESS_EVERY: usize = 200;
                    let mut total = 0usize;
                    let progress_tx = tx.clone();
                    let caches = crate::caches::scan::collect_with_progress(&seeds, &mut || {
                        total += 1;
                        if total.is_multiple_of(PROGRESS_EVERY) {
                            let _ = progress_tx.send(Msg::ScanProgress { folders: total });
                        }
                    });
                    // Flush the final count and the completed list.
                    let _ = tx.send(Msg::ScanProgress { folders: total });
                    let _ = tx.send(Msg::SeedsLoaded { caches });
                });
            }
        }
    }
}

fn step(state: State, msg: Msg, runner: &EffectRunner) -> State {
    let (mut state, mut cmd) = update(state, msg);
    while !cmd.events.is_empty() {
        let ev = cmd.events.remove(0);
        let (next, more) = update(state, ev);
        state = next;
        cmd = cmd.and(more);
    }
    for eff in cmd.effects {
        runner.run(eff);
    }
    state
}

pub fn run_loop(
    term: &mut Term,
    mut state: State,
    initial_effects: Vec<Effect>,
) -> io::Result<(State, u64)> {
    const FRAME_BUDGET: Duration = Duration::from_millis(33);

    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<Msg>();
    let runner = EffectRunner {
        msg_tx: msg_tx.clone(),
    };

    // Dispatch any boot-time effects (e.g. the initial seed scan) before the
    // first render so their workers are already running while the spinner
    // shows up.
    for eff in initial_effects {
        runner.run(eff);
    }

    loop {
        let frame_start = Instant::now();
        term.draw(|f| view::render(&mut state, f.area(), f.buffer_mut()))?;
        if state.quit {
            break;
        }

        let deadline = frame_start + FRAME_BUDGET;
        let animating = state.loading.is_some() || state.overlay.is_some();

        loop {
            // Drain any queued background msgs eagerly without rendering between them.
            if let Ok(m) = msg_rx.try_recv() {
                state = step(state, m, &runner);
                if state.quit {
                    break;
                }
                continue;
            }

            // Animating: stay inside the frame budget so the spinner ticks at 30 fps.
            // Idle: long blocking poll, no redundant renders.
            let poll_for = if animating {
                deadline.saturating_duration_since(Instant::now())
            } else {
                Duration::from_millis(250)
            };

            match ratatui::crossterm::event::poll(poll_for) {
                Ok(true) => match ratatui::crossterm::event::read() {
                    Ok(ratatui::crossterm::event::Event::Key(k))
                        if k.kind == ratatui::crossterm::event::KeyEventKind::Press =>
                    {
                        let modal = match &state.modal {
                            Modal::DeleteConfirm => keys::ModalKind::DeleteConfirm,
                            Modal::ActiveMark(_) => keys::ModalKind::ActiveMark,
                            Modal::FilterEdit => keys::ModalKind::FilterEdit,
                            Modal::None => keys::ModalKind::None,
                        };
                        if let Some(msg) = keys::key_to_msg(k, modal, state.focus_right) {
                            state = step(state, msg, &runner);
                            if state.quit {
                                break;
                            }
                        }
                        // Re-render after every keypress for snappy input → cursor latency.
                        break;
                    }
                    // Mouse / resize / focus events: re-render so layout updates apply.
                    Ok(_) => break,
                    Err(e) => return Err(e),
                },
                Ok(false) => {
                    if animating {
                        // Advance the spinner and render the next frame.
                        state = step(state, Msg::Tick, &runner);
                        break;
                    }
                    // Idle timeout, nothing changed — keep waiting; no re-render.
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    let total = state.total_freed;
    Ok((state, total))
}
