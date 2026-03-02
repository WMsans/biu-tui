use crate::storage::Settings;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    Volume,
    LoopMode,
}

impl SettingItem {
    pub fn next(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::Volume,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::Volume,
        }
    }
}

pub struct SettingsScreen {
    pub settings: Settings,
    pub list_state: ListState,
    pub selected_item: SettingItem,
}

impl SettingsScreen {
    pub fn new(settings: Settings) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            settings,
            list_state,
            selected_item: SettingItem::Volume,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(area);

        let title = Paragraph::new("Settings")
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        let items = Self::build_items(&self.settings);
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let help = Paragraph::new("[j/k] Navigate  [h/l] Adjust  [Esc/s] Back")
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    fn build_items(settings: &Settings) -> Vec<ListItem> {
        let volume_text = format!(
            "Volume        {}  {:3}%",
            Self::format_volume_bar(settings.volume),
            settings.volume
        );
        let loop_text = format!("Loop Mode     {}", settings.loop_mode.display_name());

        vec![ListItem::new(volume_text), ListItem::new(loop_text)]
    }

    fn format_volume_bar(volume: u32) -> String {
        let bar_width = 20;
        let filled = (bar_width as f32 * (volume as f32 / 100.0)) as usize;
        let filled = filled.min(bar_width);
        let empty = bar_width - filled;

        let filled_chars: String = std::iter::repeat('█').take(filled).collect();
        let empty_chars: String = std::iter::repeat('░').take(empty).collect();
        format!("{}{}", filled_chars, empty_chars)
    }

    pub fn next_item(&mut self) {
        self.selected_item = self.selected_item.next();
        self.list_state.select(Some(self.selected_item as usize));
    }

    pub fn prev_item(&mut self) {
        self.selected_item = self.selected_item.prev();
        self.list_state.select(Some(self.selected_item as usize));
    }

    pub fn adjust_up(&mut self) {
        match self.selected_item {
            SettingItem::Volume => self.settings.volume_up(),
            SettingItem::LoopMode => self.settings.next_loop_mode(),
        }
    }

    pub fn adjust_down(&mut self) {
        match self.selected_item {
            SettingItem::Volume => self.settings.volume_down(),
            SettingItem::LoopMode => self.settings.prev_loop_mode(),
        }
    }
}
