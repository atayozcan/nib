use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::editor::Editor;

pub(crate) fn render(frame: &mut Frame<'_>, editor: &Editor) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let text_area = chunks[0];
    let status_area = chunks[1];
    let message_area = chunks[2];

    let inner_height = text_area.height.saturating_sub(2) as usize;
    let inner_width = text_area.width.saturating_sub(2) as usize;
    let cursor = editor.buffer.cursor();
    let scroll_y = cursor.line.saturating_sub(inner_height.saturating_sub(1));
    let scroll_x = display_col(&editor.buffer.line(cursor.line), cursor.col)
        .saturating_sub(inner_width.saturating_sub(1));

    let title = format!(
        " {}{} ",
        editor.buffer.path().display(),
        if editor.buffer.is_dirty() { " ●" } else { "" }
    );

    let lines: Vec<Line<'_>> = (0..editor.buffer.line_count())
        .map(|i| Line::raw(editor.buffer.line(i)))
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll_y as u16, scroll_x as u16));
    frame.render_widget(paragraph, text_area);

    let status = Line::raw(format!(
        " {ln}:{col}  {lines} lines",
        ln = cursor.line + 1,
        col = cursor.col + 1,
        lines = editor.buffer.line_count(),
    ))
    .style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(Paragraph::new(status), status_area);

    frame.render_widget(
        Paragraph::new(Line::raw(editor.message.clone())),
        message_area,
    );

    let line_text = editor.buffer.line(cursor.line);
    let cursor_col_in_view = display_col(&line_text, cursor.col).saturating_sub(scroll_x);
    let cursor_row_in_view = cursor.line.saturating_sub(scroll_y);
    frame.set_cursor_position(Position::new(
        text_area.x + 1 + cursor_col_in_view as u16,
        text_area.y + 1 + cursor_row_in_view as u16,
    ));
}

fn display_col(line: &str, grapheme_col: usize) -> usize {
    line.graphemes(true)
        .take(grapheme_col)
        .map(UnicodeWidthStr::width)
        .sum()
}
