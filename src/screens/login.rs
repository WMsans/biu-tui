use crate::api::auth::{QrCodeData, QrPollData};
use crate::ui::QrCodeWidget;
use anyhow::Result;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub enum LoginState {
    Idle,
    QrWaiting { qrcode_data: QrCodeData },
    QrScanned,
    LoggedIn,
    Error(String),
}

pub struct LoginScreen {
    pub state: LoginState,
    pub status_message: String,
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            state: LoginState::Idle,
            status_message: "Press 'r' to refresh QR code".to_string(),
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
            LoginState::QrWaiting { qrcode_data } => {
                if let Ok(qr_widget) = QrCodeWidget::new(&qrcode_data.url) {
                    f.render_widget(qr_widget, chunks[1]);
                }
            }
            _ => {
                let msg = Paragraph::new(self.status_message.clone()).alignment(Alignment::Center);
                f.render_widget(msg, chunks[1]);
            }
        }

        let help = Paragraph::new("[R] Refresh QR  [Q] Quit")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }
}
