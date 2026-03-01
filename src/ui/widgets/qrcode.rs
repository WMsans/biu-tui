use qrcode::QrCode;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

pub struct QrCodeWidget {
    code: QrCode,
}

impl QrCodeWidget {
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let code = QrCode::new(url)?;
        Ok(Self { code })
    }
}

impl Widget for QrCodeWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = self.code.width();
        let block = "█";

        let start_x = area.x + (area.width.saturating_sub(width as u16 * 2)) / 2;
        let start_y = area.y + (area.height.saturating_sub(width as u16)) / 2;

        for y in 0..width {
            for x in 0..width {
                let color = if self.code[(x, y)] == qrcode::Color::Dark {
                    Color::White
                } else {
                    Color::Black
                };

                let cell_x = start_x + (x * 2) as u16;
                let cell_y = start_y + y as u16;

                if cell_x < area.x + area.width && cell_y < area.y + area.height {
                    buf[(cell_x, cell_y)].set_symbol(block).set_fg(color);
                    if cell_x + 1 < area.x + area.width {
                        buf[(cell_x + 1, cell_y)].set_symbol(block).set_fg(color);
                    }
                }
            }
        }
    }
}
