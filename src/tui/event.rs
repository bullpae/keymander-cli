//! Terminal event handling — key, mouse, resize, tick events

use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};

/// Application event
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}

/// Event handler — polls terminal events with a tick rate
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(fps: u64) -> Self {
        let tick_rate = Duration::from_millis(1000 / fps.max(1));
        Self { tick_rate }
    }

    /// Poll for the next event (blocking with timeout)
    pub fn next(&self) -> color_eyre::Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    // On Windows, crossterm sends both Press and Release events.
                    // Only handle Press to avoid duplicate processing.
                    if key.kind == KeyEventKind::Press {
                        Ok(AppEvent::Key(key))
                    } else {
                        Ok(AppEvent::Tick)
                    }
                }
                CrosstermEvent::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
