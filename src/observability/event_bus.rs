//! Tokio broadcast event bus for cross-component notifications.

use crate::models::MemoryEvent;
use std::sync::OnceLock;
use tokio::sync::broadcast;

const DEFAULT_EVENT_BUS_CAPACITY: usize = 1024;

/// Central event bus for broadcasting memory events.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<MemoryEvent>,
}

impl EventBus {
    /// Creates a new event bus with the given buffer capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publishes an event to all subscribers (best effort).
    pub fn publish(&self, event: MemoryEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribes to the event bus.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<MemoryEvent> {
        self.sender.subscribe()
    }
}

static GLOBAL_EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

/// Returns the global event bus, initializing it on first use.
#[must_use]
pub fn global_event_bus() -> &'static EventBus {
    GLOBAL_EVENT_BUS.get_or_init(|| EventBus::new(DEFAULT_EVENT_BUS_CAPACITY))
}
