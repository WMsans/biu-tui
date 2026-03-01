use ratatui::style::{Color, Style, Stylize};

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub error: Color,
    pub success: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            accent: Color::Cyan,
            error: Color::Red,
            success: Color::Green,
        }
    }
}

impl Theme {
    pub fn title_style(&self) -> Style {
        Style::default().fg(self.accent).bold()
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }
}
