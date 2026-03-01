use crate::api::auth::QrCodeData;
use crate::ui::theme::Theme;
use crate::ui::QrCodeWidget;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub enum LoginState {
    Idle,
    QrWaiting { qrcode_data: QrCodeData },
    QrScanned { qrcode_data: QrCodeData },
    LoggedIn,
    Error(String),
}

pub struct LoginScreen {
    pub state: LoginState,
    pub status_message: String,
    theme: Theme,
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            state: LoginState::Idle,
            status_message: "Press 'r' to refresh QR code".to_string(),
            theme: Theme::default(),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        let title = Paragraph::new("Biu TUI - Login")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        match &self.state {
            LoginState::QrWaiting { qrcode_data } | LoginState::QrScanned { qrcode_data } => {
                match QrCodeWidget::new(&qrcode_data.url) {
                    Ok(qr_widget) => f.render_widget(qr_widget, chunks[1]),
                    Err(_) => {
                        let error_msg = Paragraph::new("Failed to generate QR code")
                            .alignment(Alignment::Center)
                            .style(self.theme.error_style());
                        f.render_widget(error_msg, chunks[1]);
                    }
                }
            }
            LoginState::LoggedIn => {
                let msg = Paragraph::new("Login successful!")
                    .alignment(Alignment::Center)
                    .style(self.theme.title_style());
                f.render_widget(msg, chunks[1]);
            }
            LoginState::Idle | LoginState::Error(_) => {
                let style = match &self.state {
                    LoginState::Error(_) => self.theme.error_style(),
                    _ => self.theme.normal_style(),
                };
                let msg = Paragraph::new(self.status_message.clone())
                    .alignment(Alignment::Center)
                    .style(style);
                f.render_widget(msg, chunks[1]);
            }
        }

        let help = Paragraph::new("[R] Refresh QR  [Q] Quit")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }
}
