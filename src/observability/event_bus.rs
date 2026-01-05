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

/// Filtered receiver that yields events matching a predicate.
pub struct FilteredReceiver<F> {
    receiver: broadcast::Receiver<MemoryEvent>,
    predicate: F,
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
        metrics::counter!("event_bus_publish_total").increment(1);
        let receivers = self.sender.receiver_count();
        metrics::gauge!("event_bus_receivers").set(receivers as f64);
        match self.sender.send(event) {
            Ok(_) => {
                metrics::gauge!("event_bus_queue_depth").set(self.sender.len() as f64);
            },
            Err(_) => {
                metrics::counter!("event_bus_publish_failed_total").increment(1);
            },
        }
    }

    /// Subscribes to the event bus.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<MemoryEvent> {
        metrics::counter!("event_bus_subscriptions_total").increment(1);
        metrics::gauge!("event_bus_receivers").set(self.sender.receiver_count() as f64);
        self.sender.subscribe()
    }

    /// Subscribes with a predicate to filter events by type or attributes.
    #[must_use]
    pub fn subscribe_filtered<F>(&self, predicate: F) -> FilteredReceiver<F>
    where
        F: Fn(&MemoryEvent) -> bool,
    {
        metrics::counter!("event_bus_subscriptions_total").increment(1);
        metrics::gauge!("event_bus_receivers").set(self.sender.receiver_count() as f64);
        FilteredReceiver {
            receiver: self.sender.subscribe(),
            predicate,
        }
    }

    /// Subscribes to events matching the provided event type.
    #[must_use]
    pub fn subscribe_event_type(
        &self,
        event_type: &'static str,
    ) -> FilteredReceiver<impl Fn(&MemoryEvent) -> bool> {
        self.subscribe_filtered(move |event| event.event_type() == event_type)
    }
}

impl<F> FilteredReceiver<F>
where
    F: Fn(&MemoryEvent) -> bool,
{
    /// Receives the next event that matches the predicate.
    pub async fn recv(&mut self) -> Result<MemoryEvent, broadcast::error::RecvError> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if (self.predicate)(&event) {
                        return Ok(event);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    metrics::counter!("event_bus_lagged_total").increment(skipped as u64);
                },
                Err(err) => return Err(err),
            }
        }
    }
}

static GLOBAL_EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

/// Returns the global event bus, initializing it on first use.
#[must_use]
pub fn global_event_bus() -> &'static EventBus {
    GLOBAL_EVENT_BUS.get_or_init(|| EventBus::new(DEFAULT_EVENT_BUS_CAPACITY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, EventMeta, MemoryId, MemoryEvent, Namespace};

    #[tokio::test]
    async fn test_subscribe_filtered_skips_non_matching() {
        let bus = EventBus::new(16);
        let mut filtered = bus.subscribe_event_type("captured");

        bus.publish(MemoryEvent::Retrieved {
            meta: EventMeta::with_timestamp("test", None, 1),
            memory_id: MemoryId::new("id1"),
            query: "query".into(),
            score: 0.5,
        });
        bus.publish(MemoryEvent::Captured {
            meta: EventMeta::with_timestamp("test", None, 2),
            memory_id: MemoryId::new("id2"),
            namespace: Namespace::Decisions,
            domain: Domain {
                organization: None,
                project: None,
                repository: None,
            },
            content_length: 10,
        });

        let event = filtered.recv().await.expect("receive event");
        assert_eq!(event.event_type(), "captured");
    }
}
