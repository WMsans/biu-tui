use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::*,
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            running: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            self.terminal.draw(|f| {
                let area = f.area();
                f.render_widget(
                    ratatui::widgets::Paragraph::new("Biu TUI - Press 'q' to quit"),
                    area,
                );
            })?;

            if crossterm::event::poll(std::time::Duration::from_millis(100))? {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    if key.code == crossterm::event::KeyCode::Char('q') {
                        self.running = false;
                    }
                }
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}
