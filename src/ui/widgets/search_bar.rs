use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct SearchBar<'a> {
    query: &'a str,
    cursor_position: usize,
}

impl<'a> SearchBar<'a> {
    pub fn new(query: &'a str, cursor_position: usize) -> Self {
        Self {
            query,
            cursor_position,
        }
    }

    pub fn render(self, f: &mut Frame, area: Rect) {
        let mut display_text = format!("Search: {}", self.query);

        if area.width as usize > display_text.len() + 1 {
            display_text.push('_');
        }

        let search_bar = Paragraph::new(display_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(search_bar, area);
    }
}
