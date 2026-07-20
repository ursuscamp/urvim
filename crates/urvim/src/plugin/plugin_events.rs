use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bearscript::Value;

use super::api::validate_cross_plugin_value;

/// A deferred custom event emitted by a plugin.
#[derive(Clone, Debug)]
pub struct PluginEvent {
    pub(in crate::plugin) publisher: String,
    pub(in crate::plugin) event: String,
    pub(in crate::plugin) value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PluginEventTarget {
    publisher: String,
    event: String,
}

/// Shared subscription registry and queue for cross-plugin custom events.
#[derive(Default)]
pub(in crate::plugin) struct PluginEventBus {
    subscriptions: RefCell<BTreeMap<String, BTreeMap<PluginEventTarget, BTreeSet<u64>>>>,
    pending: RefCell<VecDeque<PluginEvent>>,
}

impl PluginEventBus {
    pub(in crate::plugin) fn subscribe(
        &self,
        subscriber: &str,
        publisher: String,
        event: String,
        subscription_id: u64,
    ) -> Result<(), String> {
        urvim_plugin::validate_contribution_name(&publisher, "event publisher")?;
        urvim_plugin::validate_contribution_name(&event, "plugin event name")?;
        self.subscriptions
            .borrow_mut()
            .entry(subscriber.to_string())
            .or_default()
            .entry(PluginEventTarget { publisher, event })
            .or_default()
            .insert(subscription_id);
        Ok(())
    }

    pub(in crate::plugin) fn unsubscribe(&self, subscriber: &str, subscription_id: u64) -> bool {
        let mut subscriptions = self.subscriptions.borrow_mut();
        let Some(targets) = subscriptions.get_mut(subscriber) else {
            return false;
        };

        let mut removed = false;
        targets.retain(|_, ids| {
            removed |= ids.remove(&subscription_id);
            !ids.is_empty()
        });
        if targets.is_empty() {
            subscriptions.remove(subscriber);
        }
        removed
    }

    pub(in crate::plugin) fn emit(
        &self,
        publisher: &str,
        event: String,
        value: Value,
    ) -> Result<(), String> {
        urvim_plugin::validate_contribution_name(&event, "plugin event name")?;
        validate_cross_plugin_value(&value, "plugin event value")?;
        self.pending.borrow_mut().push_back(PluginEvent {
            publisher: publisher.to_string(),
            event,
            value,
        });
        Ok(())
    }

    pub(in crate::plugin) fn take_batch(&self) -> Vec<PluginEvent> {
        self.pending.borrow_mut().drain(..).collect()
    }

    pub(in crate::plugin) fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

    pub(in crate::plugin) fn targets(&self, event: &PluginEvent) -> Vec<(String, u64)> {
        let target = PluginEventTarget {
            publisher: event.publisher.clone(),
            event: event.event.clone(),
        };
        self.subscriptions
            .borrow()
            .iter()
            .filter(|(subscriber, _)| subscriber.as_str() != event.publisher)
            .flat_map(|(subscriber, targets)| {
                targets.get(&target).into_iter().flat_map(move |ids| {
                    ids.iter().copied().map(move |id| (subscriber.clone(), id))
                })
            })
            .collect()
    }

    pub(in crate::plugin) fn remove_plugin(&self, plugin: &str) {
        self.subscriptions.borrow_mut().remove(plugin);
        self.pending
            .borrow_mut()
            .retain(|event| event.publisher != plugin);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn queues_events_and_returns_targets_in_stable_order() {
        let bus = PluginEventBus::default();
        bus.subscribe("z-listener", "host".to_string(), "changed".to_string(), 2)
            .unwrap();
        bus.subscribe("a-listener", "host".to_string(), "changed".to_string(), 3)
            .unwrap();
        bus.subscribe("host", "host".to_string(), "changed".to_string(), 1)
            .unwrap();
        bus.emit("host", "changed".to_string(), Value::Bool(true))
            .unwrap();

        let events = bus.take_batch();
        assert_eq!(events.len(), 1);
        assert_eq!(
            bus.targets(&events[0]),
            vec![("a-listener".to_string(), 3), ("z-listener".to_string(), 2)]
        );
    }

    #[test]
    fn rejects_invalid_nested_values() {
        let bus = PluginEventBus::default();
        let value = Value::Map(HashMap::from([("bad".to_string(), Value::Range(1.0, 2.0))]).into());

        assert!(bus.emit("host", "changed".to_string(), value).is_err());
        assert!(!bus.has_pending());
    }

    #[test]
    fn removing_plugin_removes_its_subscriptions_and_events() {
        let bus = PluginEventBus::default();
        bus.subscribe("listener", "host".to_string(), "changed".to_string(), 1)
            .unwrap();
        bus.emit("listener", "changed".to_string(), Value::Null)
            .unwrap();
        bus.remove_plugin("listener");

        assert!(!bus.has_pending());
        assert!(!bus.unsubscribe("listener", 1));
    }
}
