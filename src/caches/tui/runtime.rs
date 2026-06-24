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
                    let children = crate::caches::scan::enumerate_seed(&parent_path);
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
                            Err(_) => {}
                        }
                    }
                    let _ = tx.send(Msg::DeleteCompleted {
                        freed,
                        deleted_count,
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

pub fn run_loop(term: &mut Term, mut state: State) -> io::Result<(State, u64)> {
    const FRAME_BUDGET: Duration = Duration::from_millis(33);

    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<Msg>();
    let runner = EffectRunner {
        msg_tx: msg_tx.clone(),
    };

    loop {
        let frame_start = Instant::now();
        term.draw(|f| view::render(&mut state, f.area(), f.buffer_mut()))?;
        if state.quit {
            break;
        }

        let deadline = frame_start + FRAME_BUDGET;
        let animating = state.loading.is_some() || state.overlay.is_some();

        loop {
            if let Ok(m) = msg_rx.try_recv() {
                state = step(state, m, &runner);
                if state.quit {
                    break;
                }
                continue;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            let poll_for = if animating {
                remaining
            } else if remaining == Duration::ZERO {
                Duration::ZERO
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
                            Modal::None | Modal::QuitConfirm => keys::ModalKind::None,
                        };
                        if let Some(msg) = keys::key_to_msg(k, modal, state.focus_right) {
                            state = step(state, msg, &runner);
                            if state.quit {
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => return Err(e),
                },
                Ok(false) => {
                    if animating {
                        state = step(state, Msg::Tick, &runner);
                    }
                    break;
                }
                Err(e) => return Err(e),
            }

            if Instant::now() >= deadline {
                break;
            }
        }
    }

    let total = state.total_freed;
    Ok((state, total))
}
