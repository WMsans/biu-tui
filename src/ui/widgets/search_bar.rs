use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// A search bar widget that displays a query string with a cursor.
///
/// The search bar shows "Search: {query}" with a cursor underscore at the
/// specified cursor position within the query string.
#[derive(Debug)]
pub struct SearchBar<'a> {
    query: &'a str,
    cursor_position: usize,
}

impl<'a> SearchBar<'a> {
    /// Creates a new SearchBar with the given query and cursor position.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string to display
    /// * `cursor_position` - The byte index position of the cursor within the query
    pub fn new(query: &'a str, cursor_position: usize) -> Self {
        Self {
            query,
            cursor_position,
        }
    }

    /// Renders the search bar widget to the given frame at the specified area.
    ///
    /// The display shows "Search: {query}" with a cursor underscore positioned
    /// at the cursor_position within the query string.
    pub fn render(self, f: &mut Frame, area: Rect) {
        let prefix = "Search: ";
        let prefix_len = prefix.len();

        let cursor_char_pos = self.query[..self.cursor_position].chars().count();

        let mut display_text = String::with_capacity(prefix_len + self.query.len() + 1);
        display_text.push_str(prefix);

        for (i, ch) in self.query.chars().enumerate() {
            if i == cursor_char_pos && area.width as usize > display_text.len() + 1 {
                display_text.push('_');
            }
            display_text.push(ch);
        }

        if cursor_char_pos == self.query.chars().count()
            && area.width as usize > display_text.len() + 1
        {
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
