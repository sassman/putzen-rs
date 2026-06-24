//! Render the whole screen. Pure function from State → frame buffer.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, StatefulWidget, Widget,
    },
};

use super::widgets::Theme;
use super::{Modal, RunOutcome, State};
use crate::caches::format::{human_age, human_count, human_date, human_int, human_size};

const THEME: Theme = Theme::GRUVBOX;

/// Activity histogram bucket upper bounds in seconds: <1d, <1w, <1mo,
/// <3mo, <6mo, <1y, <3y, ≥3y.
pub(super) const ACTIVITY_BUCKETS: [u64; 8] = [
    86_400,
    604_800,
    2_592_000,
    7_776_000,
    15_552_000,
    31_536_000,
    94_608_000,
    u64::MAX,
];
/// Spark-bar glyphs from shortest to tallest.
pub(super) const SPARKS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub fn render(state: &mut State, area: Rect, buf: &mut Buffer) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1618, 2618), Constraint::Ratio(1000, 2618)])
        .split(outer[0]);

    render_left(state, body[0], buf);
    render_right(state, body[1], buf);
    render_footer_status(&*state, outer[1], buf);
    render_footer_keys(&*state, outer[2], buf);
    render_modal(&*state, area, buf);
    render_active_mark_modal(&*state, area, buf);
    render_loading_modal(&*state, area, buf);
    if let Some(ov) = state.overlay.as_ref() {
        draw_result(&ov.outcome, area, buf);
    }
}

fn render_loading_modal(state: &State, area: Rect, buf: &mut Buffer) {
    let Some(l) = state.loading.as_ref() else {
        return;
    };
    let body_style = THEME.modal_body_style();
    let block_style = THEME.modal_block_style();

    let spinner = format!("{}  {}", l.glyph(), l.label);
    let elapsed_secs = l.started.elapsed().as_secs();
    let elapsed_line = if elapsed_secs > 0 {
        format!("{elapsed_secs}s elapsed")
    } else {
        String::new()
    };

    let mut lines = vec![
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            spinner,
            body_style.add_modifier(Modifier::BOLD),
        )),
    ];
    if !elapsed_line.is_empty() {
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(elapsed_line, THEME.dim_style())));
    }

    let h = (lines.len() as u16 + 2).min(area.height).max(5);
    let w = area.width.min(60);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let modal = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    Clear.render(modal, buf);
    Paragraph::new(lines)
        .style(body_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(block_style)
                .style(body_style)
                .title(Span::styled(
                    " Loading ",
                    block_style.add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center),
        )
        .render(modal, buf);
}

/// Render a centred result modal that summarises a delete pass. Shown for
/// ~2 seconds inside the TUI before tearing down the alternate screen.
pub fn draw_result(outcome: &RunOutcome, area: Rect, buf: &mut Buffer) {
    let w = area.width.min(60);
    let h = area.height.min(7);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let modal = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let body_style = THEME.modal_body_style();
    let block_style = THEME.modal_block_style();

    let title_text = if outcome.dry_run {
        " Dry run "
    } else {
        " Done "
    };
    let line = if outcome.dry_run {
        format!(
            "Would free {} across {} caches",
            human_size(outcome.freed),
            outcome.deleted,
        )
    } else {
        format!(
            "Freed {} across {} caches",
            human_size(outcome.freed),
            outcome.deleted,
        )
    };

    Clear.render(modal, buf);
    Paragraph::new(vec![
        Line::from(Span::raw("")),
        Line::from(Span::styled(line, body_style.add_modifier(Modifier::BOLD))),
    ])
    .style(body_style)
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(block_style)
            .style(body_style)
            .title(Span::styled(
                title_text,
                block_style.add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center),
    )
    .render(modal, buf);
}

fn render_active_mark_modal(state: &State, area: Rect, buf: &mut Buffer) {
    if !matches!(state.modal, Modal::ActiveMark(_)) {
        return;
    }
    let w = area.width.min(64);
    let h = area.height.min(9);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let modal = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let body_style = THEME.modal_body_style();
    let block_style = THEME.modal_block_style();
    let key_style = THEME.gutter_active_style();

    let n_days = state.floor.floor.as_secs() / 86_400;

    let lines = vec![
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            format!("The cache folder age is < {n_days} days,"),
            body_style,
        )),
        Line::from(Span::styled(
            "so that cache seems to be active.",
            body_style,
        )),
        Line::from(Span::styled(
            "Sure marking it for deletion?",
            body_style.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  [", body_style),
            Span::styled("y", key_style),
            Span::styled("] yes   ", body_style),
            Span::styled("[", body_style),
            Span::styled("N", key_style),
            Span::styled("] cancel", body_style),
        ]),
    ];

    Clear.render(modal, buf);
    Paragraph::new(lines)
        .style(body_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(block_style)
                .style(body_style)
                .title(Span::styled(
                    " Confirm marking active cache ",
                    block_style.add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center),
        )
        .render(modal, buf);
}

fn render_modal(state: &State, area: Rect, buf: &mut Buffer) {
    if !matches!(state.modal, Modal::DeleteConfirm) {
        return;
    }

    let body_style = THEME.modal_body_style();
    let dim_style = THEME.dim_style();
    let block_style = THEME.modal_block_style();
    let key_style = THEME.gutter_active_style();

    let total: u64 = state
        .marks
        .marked
        .iter()
        .filter_map(|&i| state.all.get(i).map(|c| c.size_bytes))
        .sum();
    let count = state.marks.count();
    const MAX_LIST: usize = 3;

    let mut lines: Vec<Line> = vec![Line::from(Span::raw(""))];

    if count <= MAX_LIST {
        // Few enough: list each cache and a Total row.
        for &i in state.marks.marked.iter() {
            if let Some(c) = state.all.get(i) {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}  ", c.label), body_style),
                    Span::styled(human_size(c.size_bytes), dim_style),
                ]));
            }
        }
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(vec![
            Span::styled("Total: ", body_style),
            Span::styled(human_size(total), body_style.add_modifier(Modifier::BOLD)),
        ]));
    } else {
        // Too many to fit; summarise.
        lines.push(Line::from(vec![
            Span::styled(format!("{count} caches · "), body_style),
            Span::styled(human_size(total), body_style.add_modifier(Modifier::BOLD)),
        ]));
    }

    lines.push(Line::from(Span::raw("")));
    // Enter (and `y`) confirms — Y is uppercase to signal the default.
    lines.push(Line::from(vec![
        Span::styled("[", body_style),
        Span::styled("Y", key_style),
        Span::styled("] yes   ", body_style),
        Span::styled("[", body_style),
        Span::styled("n", key_style),
        Span::styled("] cancel", body_style),
    ]));
    if state.dry_run {
        lines.push(Line::from(Span::styled(
            "no files will be touched",
            dim_style,
        )));
    }

    // Modal sized to fit the chosen content.
    let h = (lines.len() as u16 + 2).min(area.height).max(5);
    let w = area.width.min(60);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let modal = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let title_text = if state.dry_run {
        format!(" Delete {count} caches? (dry run) ")
    } else {
        format!(" Delete {count} caches? ")
    };

    Clear.render(modal, buf);
    Paragraph::new(lines)
        .style(body_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(block_style)
                .style(body_style)
                .title(Span::styled(
                    title_text,
                    block_style.add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center),
        )
        .render(modal, buf);
}

/// Width of the right-side padding inside the left panel block, kept here
/// so `col_widths` and the actual `Block::padding(...)` stay in sync.
pub(super) const LEFT_PANEL_RIGHT_PAD: u16 = 1;

pub(super) fn col_widths(area_width: u16) -> (usize, usize, usize, usize) {
    // Inner = outer area - block borders (2) - right padding so AGE doesn't
    // get clipped against the scrollbar overlay.
    let inner = (area_width as usize).saturating_sub(2 + LEFT_PANEL_RIGHT_PAD as usize);
    // Budget for content cells:
    //   gutter (2) + name + " " + score + " " + size + " " + age = inner
    // So three inter-column separators (3 cells) come off the top too.
    let budget = inner.saturating_sub(2 + 3);

    // NAME: 1/3 of the budget.
    let name = (budget as f32 / 3.0).round() as usize;

    // SIZE and AGE: pinned minimums (real values need 7 and 5 cells).
    let size_w = 7;
    let age_w = 5;

    // SCORE: gets whatever is left in the 2/3 portion.
    let score = budget.saturating_sub(name + size_w + age_w);

    // Sane floors on very narrow terminals.
    let name = name.max(8);
    let score = score.max(4);
    (name, score, size_w, age_w)
}

fn render_left(state: &mut State, area: Rect, buf: &mut Buffer) {
    let (name_w, score_w, size_w, age_w) = col_widths(area.width);

    let indices = state.sorted_indices();
    let header_style = THEME.header_style();
    let body_style = THEME.body_style();
    let active_style = THEME.gutter_active_style();
    let marked_style = THEME.gutter_marked_style();
    let hot_bar_style = Style::default().fg(THEME.hot);
    let ok_bar_style = Style::default().fg(THEME.ok);

    let max_score = indices
        .iter()
        .map(|&i| state.all[i].score(state.now))
        .fold(0f64, f64::max)
        .max(1e-9);

    let header_line = Line::styled(
        format!(
            "  {:<nw$} {:<sw$} {:>zw$} {:>aw$}",
            "NAME",
            "SCORE",
            "SIZE",
            "AGE",
            nw = name_w,
            sw = score_w,
            zw = size_w,
            aw = age_w,
        ),
        header_style,
    );

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .map(|(visible_row, &idx)| {
            let c = &state.all[idx];
            let active = visible_row == state.cursor;
            let marked = state.marks.is_marked(idx);
            // When a row is BOTH active and marked, paint the `●` in the active
            // colour so the cursor stays visible instead of being hidden under
            // the marked-orange dot.
            let gutter = match (marked, active) {
                (true, true) => Span::styled("● ", active_style),
                (true, false) => Span::styled("● ", marked_style),
                (false, true) => Span::styled("┃ ", active_style),
                (false, false) => Span::raw("  "),
            };
            let age = c
                .age(state.now)
                .map(human_age)
                .unwrap_or_else(|| "—".into());
            let size_str = human_size(c.size_bytes);

            // Right-anchor size + age. If their actual width exceeds the planned
            // column widths (e.g. "1019 KiB" overflows the 7-cell size slot),
            // grow LEFT by eating into the score bar's width — so age never gets
            // pushed off the right edge of the pane.
            let size_extra = size_str.chars().count().saturating_sub(size_w);
            let age_extra = age.chars().count().saturating_sub(age_w);
            let score_eff = score_w.saturating_sub(size_extra + age_extra).max(1);

            let score = c.score(state.now);
            let cells = if c.newest_mtime.is_none() {
                0
            } else {
                ((score / max_score) * score_eff as f64).round() as usize
            }
            .min(score_eff);
            let bar = "█".repeat(cells);
            let is_active = state.floor.is_active(c.age(state.now));
            let bar_style = if is_active {
                ok_bar_style
            } else {
                hot_bar_style
            };

            // On the active row, tint name/size/age yellow but keep the bar at
            // its tier colour (hot/ok). The list-wide highlight only paints bg,
            // so per-span fg wins and the bar stays red/green.
            let text_style = if active {
                Style::default().fg(THEME.gutter_active)
            } else {
                body_style
            };

            ListItem::new(Line::from(vec![
                gutter,
                Span::styled(format!("{:<nw$} ", c.label, nw = name_w), text_style),
                Span::styled(format!("{:<sw$} ", bar, sw = score_eff), bar_style),
                // Right-align size + age. Shorts get left-padded to their min
                // width; longs render unpadded but `score_eff` was shrunk above
                // to keep the line aligned to the right edge.
                Span::styled(format!("{:>zw$} ", size_str, zw = size_w), text_style),
                Span::styled(format!("{:>aw$}", age, aw = age_w), text_style),
            ]))
        })
        .collect();

    let title = if state.stack_labels.is_empty() {
        " putzen caches — ranked ".to_string()
    } else {
        format!(
            " putzen caches — ranked — {} ",
            state.stack_labels.join(" > ")
        )
    };

    // Draw the block + borders + title first, then split the inner area into
    // a 1-row header and the scrollable list body. When the right pane has
    // focus, paint the left border in the active gutter colour to make the
    // focus visible at a glance.
    let border_style = if !state.focus_right {
        Style::default().fg(THEME.gutter_active).bg(THEME.bg)
    } else {
        THEME.block_style()
    };
    // 1-cell right padding so list content doesn't touch the scrollbar
    // overlaid on the right border. Mirrored in `col_widths` via
    // `LEFT_PANEL_RIGHT_PAD` so AGE stays inside the rendered area.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(body_style)
        .padding(ratatui::widgets::Padding::right(LEFT_PANEL_RIGHT_PAD))
        .title(Span::styled(title, THEME.title_style()));
    let inner = block.inner(area);
    block.render(area, buf);

    // Reserve a 1-row strip at the bottom for the filter, if any.
    let filter_present = state.filter.is_some();
    let constraints: Vec<Constraint> = if filter_present {
        vec![
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]
    } else {
        vec![Constraint::Length(1), Constraint::Min(1)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    Paragraph::new(header_line)
        .style(body_style)
        .render(chunks[0], buf);

    if filter_present {
        render_filter_strip(state, chunks[2], buf);
    }

    // Build a local ListState so ratatui auto-scrolls to keep the cursor visible.
    let mut left_ls = ratatui::widgets::ListState::default();
    left_ls.select(Some(state.cursor));

    // Subtle bg + yellow-ish fg on the active row so it pops across every
    // column. Per-span colours that already specify an fg (the score bar's
    // hot/warm/ok tier, the gutter, etc.) keep their own colour.
    let visible_len = state.sorted_indices().len();

    // bg-only highlight: keeps each span's explicit fg (notably the score
    // bar's hot/ok tier) intact, while still painting the active row's
    // background subtly so the cursor is readable across all columns.
    let list = List::new(items).highlight_style(Style::default().bg(THEME.bg_sel));
    StatefulWidget::render(list, chunks[1], buf, &mut left_ls);

    // Overlay a vertical scrollbar ON the right border of the block, but
    // only spanning the LIST body — not the column header above nor the
    // bottom border (or filter strip) below. The border line itself acts
    // as the track; the thumb overdraws it with a solid block.
    if visible_len > chunks[1].height as usize {
        let mut sb_state = ScrollbarState::new(visible_len)
            .position(state.cursor.min(visible_len.saturating_sub(1)));
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("█")
            .thumb_style(Style::default().fg(THEME.gutter_active));
        let sb_area = Rect {
            x: area.x,
            y: chunks[1].y,
            width: area.width,
            height: chunks[1].height,
        };
        StatefulWidget::render(scrollbar, sb_area, buf, &mut sb_state);
    }
}

/// Compute two rows of spark glyphs for the temporal distribution of file
/// mtimes in `c.top_files`. Each of the 8 buckets occupies a 4-cell slot
/// (spark + 3 spaces) so the bar aligns under the axis labels with breathing
/// room. The bar is rendered across two rows, giving it 16 visual levels.
fn activity_sparkline_rows(
    c: &crate::caches::model::Cache,
    now: std::time::SystemTime,
) -> [Vec<Span<'static>>; 2] {
    let mut counts = [0u32; 8];
    for tf in &c.top_files {
        let Some(m) = tf.mtime else { continue };
        let age = now.duration_since(m).unwrap_or_default().as_secs();
        for (i, &upper) in ACTIVITY_BUCKETS.iter().enumerate() {
            if age < upper {
                counts[i] += 1;
                break;
            }
        }
    }
    // Reverse so the axis reads left=old → right=recent.
    counts.reverse();
    let max = counts.iter().copied().max().unwrap_or(0);
    let bar_style = Style::default().fg(THEME.hot);
    let dim = THEME.dim_style();
    if max == 0 {
        return [vec![], vec![Span::styled("no mtime data", dim)]];
    }
    let mut top: Vec<Span<'static>> = Vec::with_capacity(16);
    let mut bot: Vec<Span<'static>> = Vec::with_capacity(16);
    for &n in &counts {
        // Map count onto a 0..=16 height (two rows × 8 partial levels).
        let h = ((n as u64 * 16 / max as u64) as usize).min(16);
        let (top_glyph, bot_glyph) = if h == 0 {
            (" ", " ")
        } else if h <= 8 {
            (" ", SPARKS[h - 1])
        } else {
            (SPARKS[h - 9], SPARKS[7])
        };
        top.push(Span::styled(top_glyph, bar_style));
        top.push(Span::raw("   "));
        bot.push(Span::styled(bot_glyph, bar_style));
        bot.push(Span::raw("   "));
    }
    [top, bot]
}

fn render_right(state: &mut State, area: Rect, buf: &mut Buffer) {
    let indices = state.sorted_indices();
    let body_style = THEME.body_style();
    let dim_style = THEME.dim_style();
    let header_style = THEME.header_style();

    // Draw the bordered block first; we render header + list inside.
    // Padding pulls content one cell off each border for breathing room.
    // When this pane has focus, paint its border in the active gutter colour
    // so the user can see where Up/Down will go.
    let border_style = if state.focus_right {
        Style::default().fg(THEME.gutter_active).bg(THEME.bg)
    } else {
        THEME.block_style()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(body_style)
        .padding(ratatui::widgets::Padding::uniform(1))
        .title(Span::styled(" details ", THEME.title_style()));
    let inner = block.inner(area);
    block.render(area, buf);

    let Some(&idx) = indices.get(state.cursor) else {
        Paragraph::new(Line::from(Span::styled("no caches", dim_style)))
            .style(body_style)
            .render(inner, buf);
        return;
    };

    let c = &state.all[idx];
    let age = c
        .age(state.now)
        .map(human_age)
        .unwrap_or_else(|| "—".into());
    let touched = c.newest_mtime.map(human_date).unwrap_or_else(|| "—".into());

    let mut header_lines = vec![
        Line::from(Span::styled(c.label.clone(), THEME.title_style())),
        Line::from(Span::styled(c.path.display().to_string(), dim_style)),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("Size         ", dim_style),
            Span::styled(human_size(c.size_bytes), body_style),
        ]),
        Line::from(vec![
            Span::styled("Age          ", dim_style),
            Span::styled(age, body_style),
        ]),
        Line::from(vec![
            Span::styled("Score        ", dim_style),
            Span::styled(human_count(c.score(state.now)), body_style),
        ]),
        Line::from(vec![
            Span::styled("Files        ", dim_style),
            Span::styled(human_int(c.file_count), body_style),
        ]),
        Line::from(vec![
            Span::styled("Dirs         ", dim_style),
            Span::styled(human_int(c.dir_count), body_style),
        ]),
        Line::from(vec![
            Span::styled("Last touched ", dim_style),
            Span::styled(touched, body_style),
        ]),
    ];

    if c.unreadable > 0 {
        header_lines.push(Line::from(Span::styled(
            format!("partial: {} entries unreadable", c.unreadable),
            dim_style,
        )));
    }

    header_lines.push(Line::from(Span::raw("")));
    header_lines.push(Line::from(Span::styled("Activity", header_style)));
    let [top_row, bot_row] = activity_sparkline_rows(c, state.now);
    if !top_row.is_empty() {
        header_lines.push(Line::from(top_row));
    }
    header_lines.push(Line::from(bot_row));
    // Axis labels: 4-cell-wide buckets, oldest on the left, most-recent on
    // the right. Matches the reversed bar above. Leftmost slot is the
    // open-ended ≥3y bucket, written `3y+`.
    //   `3y+ 3y  1y  6mo 3mo 1mo 1w  1d  `
    header_lines.push(Line::from(Span::styled(
        "3y+ 3y  1y  6mo 3mo 1mo 1w  1d  ",
        dim_style,
    )));

    header_lines.push(Line::from(Span::raw("")));
    header_lines.push(Line::from(Span::styled("Files (by size)", header_style)));

    let header_h = header_lines.len() as u16;

    // Split inner area: header on top, files list below.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_h), Constraint::Min(0)])
        .split(inner);

    Paragraph::new(header_lines)
        .style(body_style)
        .render(chunks[0], buf);

    // Right-align size in a 9-char column; truncate name to fit the rest.
    let inner_w = inner.width as usize;
    let size_w = 9usize;
    let name_w = inner_w.saturating_sub(size_w + 1).max(8);

    let items: Vec<ListItem> = c
        .top_files
        .iter()
        .map(|tf| {
            let mut name = tf.name.clone();
            if name.chars().count() > name_w {
                let truncated: String = name.chars().take(name_w.saturating_sub(1)).collect();
                name = format!("{truncated}…");
            }
            let size = human_size(tf.size_bytes);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<nw$} ", name, nw = name_w), body_style),
                Span::styled(format!("{:>sw$}", size, sw = size_w), dim_style),
            ]))
        })
        .collect();

    // When this pane has focus, show a yellow selection indicator + subtle
    // bg highlight on the active file so the user can see where Up/Down
    // points. When unfocused, no highlight.
    let (highlight_style, highlight_symbol) = if state.focus_right {
        (
            Style::default().fg(THEME.gutter_active).bg(THEME.bg_sel),
            "┃ ",
        )
    } else {
        (Style::default(), "  ")
    };
    let list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);
    let mut right_ls = ratatui::widgets::ListState::default();
    right_ls.select(Some(state.files_cursor));
    StatefulWidget::render(list, chunks[1], buf, &mut right_ls);
}

fn render_filter_strip(state: &State, area: Rect, buf: &mut Buffer) {
    let Some(f) = state.filter.as_ref() else {
        return;
    };
    let dim = THEME.dim_style();
    let active = Style::default().fg(THEME.gutter_active);
    let body = THEME.body_style();

    let spans: Vec<Span> = if matches!(state.modal, Modal::FilterEdit) {
        vec![
            Span::styled("/", active),
            Span::styled(f.input.clone(), body),
            Span::styled("█  ", active),
            Span::styled("(Enter to apply, Esc to cancel)", dim),
        ]
    } else {
        let n = state.sorted_indices().len();
        vec![
            Span::styled("/", active),
            Span::styled(f.input.clone(), body),
            Span::styled("   ", body),
            Span::styled(format!("({n} matches  ·  "), dim),
            Span::styled("[*]", active),
            Span::styled(" mark all  ·  ", dim),
            Span::styled("[/]", active),
            Span::styled(" edit)", dim),
        ]
    };
    Paragraph::new(Line::from(spans)).render(area, buf);
}

fn render_footer_status(state: &State, area: Rect, buf: &mut Buffer) {
    let total: u64 = state
        .marks
        .marked
        .iter()
        .filter_map(|&i| state.all.get(i).map(|c| c.size_bytes))
        .sum();
    let body_style = THEME.body_style();
    let dim_style = THEME.dim_style();

    let visible = state.sorted_indices();
    let preview_bytes: u64 = visible
        .iter()
        .take(state.cursor + 1)
        .filter(|&&i| !state.marks.is_marked(i))
        .filter_map(|&i| state.all.get(i).map(|c| c.size_bytes))
        .sum();
    let preview_count: usize = visible
        .iter()
        .take(state.cursor + 1)
        .filter(|&&i| !state.marks.is_marked(i))
        .count();

    let mut spans = vec![
        Span::styled("● ", THEME.gutter_marked_style()),
        Span::styled(
            format!(
                "Marked {} caches · {} to free",
                state.marks.count(),
                human_size(total)
            ),
            body_style,
        ),
    ];
    if preview_count > 0 {
        spans.push(Span::styled("    ↓ [m] would mark ", dim_style));
        spans.push(Span::styled(format!("{preview_count}"), body_style));
        spans.push(Span::styled(" more (+", dim_style));
        spans.push(Span::styled(human_size(preview_bytes), body_style));
        spans.push(Span::styled(")", dim_style));
    }
    Paragraph::new(Line::from(spans))
        .style(body_style)
        .render(area, buf);
}

fn render_footer_keys(state: &State, area: Rect, buf: &mut Buffer) {
    let dim = THEME.dim_style();
    let editing = matches!(state.modal, Modal::FilterEdit);
    let text = if editing {
        "[Enter] apply filter  [Esc] cancel  [Backspace] erase"
    } else if state.focus_right {
        "[↑↓/jk] scroll files  [Tab/Esc] back to list  [q] quit"
    } else if state.filter.is_some() {
        "[↑↓/jk] move  [/] edit filter  [*] mark all  [Space] mark  [m] mark-to  [s] sort  [d] delete  [q] quit"
    } else {
        "[↑↓/jk] move  [Tab] focus  [/] filter  [Space] mark  [m] mark-to  [s] sort  [→/l/Enter] drill  [←/h/Esc] back  [d] delete  [q] quit"
    };
    Paragraph::new(Line::from(Span::styled(text, dim)))
        .style(Style::default())
        .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caches::model::{Cache, FloorPolicy, MarkSet, Sort};
    use crate::caches::tui::State;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn fixture() -> State {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        State {
            now,
            all: vec![
                Cache {
                    label: "alpha".into(),
                    path: PathBuf::from("/x/alpha"),
                    size_bytes: 2_000_000_000,
                    newest_mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(0)),
                    file_count: 10,
                    dir_count: 0,
                    top_files: Vec::new(),
                    unreadable: 0,
                },
                Cache {
                    label: "beta".into(),
                    path: PathBuf::from("/x/beta"),
                    size_bytes: 50_000_000,
                    newest_mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(90 * 86_400)),
                    file_count: 4,
                    dir_count: 1,
                    top_files: Vec::new(),
                    unreadable: 0,
                },
            ],
            sort: Sort::Score,
            marks: MarkSet::default(),
            cursor: 0,
            files_cursor: 0,
            floor: FloorPolicy {
                floor: Duration::from_secs(7 * 86_400),
            },
            focus_right: false,
            stack: Vec::new(),
            stack_labels: Vec::new(),
            quit: false,
            modal: crate::caches::tui::Modal::None,
            dry_run: false,
            yes_mode: false,
            total_freed: 0,
            filter: None,
            loading: None,
            overlay: None,
            level_dirty: false,
            drill_paths: Vec::new(),
        }
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn col_widths_match_one_third_for_typical_pane() {
        // Pane on a 100-col terminal at golden split ≈ 62 cols
        let (name, score, size, age) = col_widths(62);
        assert_eq!(size, 7);
        assert_eq!(age, 5);
        // budget = 62 - 2 (borders) - 1 (right pad) - 2 (gutter) - 3 (column separators) = 54
        // name = round(54 / 3) = 18
        assert_eq!(name, 18, "NAME should be ~1/3 of the budget, got {}", name);
        // score takes the slack; name + score + size + age fill the budget
        assert_eq!(name + score + size + age, 54);
    }

    #[test]
    fn col_widths_floor_for_narrow_pane() {
        let (name, score, _, _) = col_widths(20);
        assert!(name >= 8);
        assert!(score >= 4);
    }

    #[test]
    fn render_shows_both_entries_and_active_gutter() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let buf = term.backend().buffer().clone();
        let dump = buffer_to_string(&buf);
        assert!(dump.contains("alpha"), "alpha row missing:\n{}", dump);
        assert!(dump.contains("beta"), "beta row missing:\n{}", dump);
        assert!(dump.contains("┃ alpha"), "active gutter missing:\n{}", dump);
    }

    #[test]
    fn render_includes_score_bar_for_positive_score() {
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("█"),
            "expected at least one bar cell `█`:\n{}",
            dump
        );
    }

    #[test]
    fn render_status_reflects_marks() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.marks.toggle(0);
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("Marked 1 caches"),
            "status missing:\n{}",
            dump
        );
    }

    #[test]
    fn right_pane_shows_score_row() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(dump.contains("Score"), "Score row missing:\n{}", dump);
    }

    #[test]
    fn modal_shows_dry_run_hints() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.marks.toggle(0);
        state.modal = crate::caches::tui::Modal::DeleteConfirm;
        state.dry_run = true;
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("dry run"),
            "dry-run title hint missing:\n{}",
            dump
        );
        assert!(
            dump.contains("no files will be touched"),
            "dry-run footer missing:\n{}",
            dump
        );
    }

    #[test]
    fn right_pane_shows_top_files() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.all[0].top_files = vec![
            crate::caches::model::TopFile {
                name: "blob.bin".into(),
                size_bytes: 1_500_000_000,
                mtime: None,
            },
            crate::caches::model::TopFile {
                name: "data.tar".into(),
                size_bytes: 50_000_000,
                mtime: None,
            },
        ];
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(dump.contains("Files (by size)"), "files header missing");
        assert!(dump.contains("blob.bin"), "biggest file missing");
    }

    #[test]
    fn right_pane_shows_partial_footnote() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.all[0].unreadable = 7;
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("partial: 7 entries unreadable"),
            "partial counter missing:\n{}",
            dump
        );
    }

    #[test]
    fn draw_result_shows_freed_summary() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let outcome = RunOutcome {
            freed: 1_500_000_000,
            deleted: 3,
            dry_run: false,
        };
        term.draw(|f| draw_result(&outcome, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(dump.contains("Freed"), "result summary missing:\n{}", dump);
        assert!(
            dump.contains("3 caches"),
            "deleted count missing:\n{}",
            dump
        );
    }

    #[test]
    fn draw_result_dry_run_shows_would_free() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let outcome = RunOutcome {
            freed: 1_000,
            deleted: 1,
            dry_run: true,
        };
        term.draw(|f| draw_result(&outcome, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("Would free"),
            "dry-run wording missing:\n{}",
            dump
        );
    }

    #[test]
    fn cursor_preview_appears_when_m_would_mark() {
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.cursor = 1; // cursor on second row; [m] would mark both rows
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("would mark 2 more"),
            "cursor preview missing:\n{}",
            dump
        );
    }

    #[test]
    fn breadcrumb_reflects_drill_stack() {
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.stack_labels.push("Library/Caches".into());
        state.stack_labels.push("Homebrew".into());
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("Library/Caches > Homebrew"),
            "breadcrumb missing:\n{}",
            dump
        );
    }

    #[test]
    fn right_pane_shows_activity_sparkline() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.all[0].top_files = vec![crate::caches::model::TopFile {
            name: "a".into(),
            size_bytes: 1,
            mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(90 * 86_400)),
        }];
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("Activity"),
            "activity header missing:\n{}",
            dump
        );
        assert!(
            SPARKS.iter().any(|&s| dump.contains(s)),
            "no spark char visible:\n{}",
            dump
        );
    }

    #[test]
    fn render_active_mark_modal_shows_floor_days() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.modal = crate::caches::tui::Modal::ActiveMark(vec![0]);
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("seems to be active"),
            "active modal text missing:\n{}",
            dump
        );
        assert!(
            dump.contains("< 7 days"),
            "floor wording missing:\n{}",
            dump
        );
    }

    #[test]
    fn modal_renders_when_delete_requested() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.marks.toggle(0);
        state.modal = crate::caches::tui::Modal::DeleteConfirm;
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(
            dump.contains("Delete 1 caches?"),
            "modal title missing:\n{}",
            dump
        );
        assert!(dump.contains("[Y] yes"), "modal Y default prompt missing");
        assert!(dump.contains("[n] cancel"), "modal n prompt missing");
    }

    #[test]
    fn render_loading_modal_shows_spinner() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut state = fixture();
        state.loading = Some(crate::caches::tui::Loading {
            label: "huggingface".into(),
            frame: 0,
            started: std::time::Instant::now(),
        });
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(dump.contains("Loading"), "loading title missing:\n{}", dump);
        assert!(
            dump.contains("huggingface"),
            "loading label missing:\n{}",
            dump
        );
    }

    #[test]
    fn many_rows_renders_without_panic_at_cursor_50() {
        use crate::caches::tui::Msg;
        let mut state = fixture();
        // Replace fixture's two rows with 100 caches.
        state.all = (0..100u64)
            .map(|i| Cache {
                label: format!("c{i:03}"),
                path: PathBuf::from(format!("/x/c{i:03}")),
                size_bytes: 1024,
                newest_mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(i * 100)),
                file_count: 1,
                dir_count: 0,
                top_files: Vec::new(),
                unreadable: 0,
            })
            .collect();

        for _ in 0..50 {
            state = crate::caches::tui::update(state, Msg::MoveDown).0;
        }
        assert_eq!(state.cursor, 50);

        // Render at a small height; row c050 must scroll into view.
        let backend = TestBackend::new(80, 10);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(&mut state, f.area(), f.buffer_mut()))
            .unwrap();
        let dump = buffer_to_string(term.backend().buffer());
        assert!(dump.contains("c050"), "row c050 not rendered:\n{dump}");
    }
}
