use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Represents a keyboard action detected by the tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Key was initially pressed
    Press(KeyCode),
    /// Key is being held (after action interval)
    Hold(KeyCode),
    /// Key was released
    Release(KeyCode),
}

/// Tracks key hold states and generates Press/Hold/Release actions.
pub struct KeyHoldTracker {
    held_keys: HashSet<KeyCode>,
    last_action: HashMap<KeyCode, Instant>,
    action_interval: Duration,
}

impl KeyHoldTracker {
    /// Creates a new tracker with the specified minimum interval between hold actions.
    pub fn new(action_interval: Duration) -> Self {
        Self {
            held_keys: HashSet::new(),
            last_action: HashMap::new(),
            action_interval,
        }
    }

    /// Processes a key event and returns the corresponding action, if any.
    pub fn process(&mut self, key: KeyEvent) -> Option<KeyAction> {
        let code = key.code;

        match key.kind {
            KeyEventKind::Press => {
                if self.held_keys.contains(&code) {
                    if let Some(last) = self.last_action.get(&code) {
                        if last.elapsed() >= self.action_interval {
                            self.last_action.insert(code, Instant::now());
                            return Some(KeyAction::Hold(code));
                        }
                    }
                    None
                } else {
                    self.held_keys.insert(code);
                    self.last_action.insert(code, Instant::now());
                    Some(KeyAction::Press(code))
                }
            }
            KeyEventKind::Release => {
                self.held_keys.remove(&code);
                self.last_action.remove(&code);
                Some(KeyAction::Release(code))
            }
            KeyEventKind::Repeat => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyModifiers};
    use std::time::Duration;

    fn make_key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::empty(), kind)
    }

    #[test]
    fn test_press_generates_press_action() {
        let mut tracker = KeyHoldTracker::new(Duration::from_millis(200));
        let key = make_key(KeyCode::Char('h'), KeyEventKind::Press);

        let action = tracker.process(key);

        assert!(matches!(action, Some(KeyAction::Press(KeyCode::Char('h')))));
    }

    #[test]
    fn test_release_generates_release_action() {
        let mut tracker = KeyHoldTracker::new(Duration::from_millis(200));
        let key = make_key(KeyCode::Char('h'), KeyEventKind::Press);

        tracker.process(key);
        let action = tracker.process(make_key(KeyCode::Char('h'), KeyEventKind::Release));

        assert!(matches!(
            action,
            Some(KeyAction::Release(KeyCode::Char('h')))
        ));
    }

    #[test]
    fn test_hold_after_interval_generates_hold_action() {
        let mut tracker = KeyHoldTracker::new(Duration::from_millis(50));
        let key = make_key(KeyCode::Char('h'), KeyEventKind::Press);

        tracker.process(key);
        std::thread::sleep(Duration::from_millis(60));
        let action = tracker.process(make_key(KeyCode::Char('h'), KeyEventKind::Press));

        assert!(matches!(action, Some(KeyAction::Hold(KeyCode::Char('h')))));
    }

    #[test]
    fn test_hold_before_interval_returns_none() {
        let mut tracker = KeyHoldTracker::new(Duration::from_millis(200));
        let key = make_key(KeyCode::Char('h'), KeyEventKind::Press);

        tracker.process(key);
        let action = tracker.process(make_key(KeyCode::Char('h'), KeyEventKind::Press));

        assert!(action.is_none());
    }

    #[test]
    fn test_repeat_returns_none() {
        let mut tracker = KeyHoldTracker::new(Duration::from_millis(200));
        let key = make_key(KeyCode::Char('h'), KeyEventKind::Repeat);

        let action = tracker.process(key);

        assert!(action.is_none());
    }
}
