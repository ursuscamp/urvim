use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::conversion::BearNumber;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plugin) enum PluginTimerKind {
    Defer,
    Timeout,
    Interval,
}

impl PluginTimerKind {
    pub(in crate::plugin) fn as_str(self) -> &'static str {
        match self {
            Self::Defer => "defer",
            Self::Timeout => "timeout",
            Self::Interval => "interval",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::plugin) struct PluginTimerEvent {
    pub(in crate::plugin) timer_id: u64,
    pub(in crate::plugin) kind: PluginTimerKind,
}

pub(in crate::plugin) struct PluginTimerRegistry {
    next_id: AtomicU64,
    timers: Mutex<HashMap<u64, PluginTimer>>,
    event_tx: Sender<PluginTimerEvent>,
    event_rx: Mutex<Receiver<PluginTimerEvent>>,
}

struct PluginTimer {
    plugin: String,
    kind: PluginTimerKind,
    cancelled: Arc<AtomicBool>,
}

impl Default for PluginTimerRegistry {
    fn default() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            next_id: AtomicU64::new(1),
            timers: Mutex::new(HashMap::new()),
            event_tx,
            event_rx: Mutex::new(event_rx),
        }
    }
}

impl PluginTimerRegistry {
    pub(in crate::plugin) fn defer(&self, plugin: &str) -> u64 {
        let id = self.insert_timer(plugin, PluginTimerKind::Defer);
        self.event_tx
            .send(PluginTimerEvent {
                timer_id: id,
                kind: PluginTimerKind::Defer,
            })
            .ok();
        id
    }

    pub(in crate::plugin) fn set_timeout(&self, plugin: &str, ms: u64) -> u64 {
        let id = self.insert_timer(plugin, PluginTimerKind::Timeout);
        let cancelled = self.cancelled_flag(id).expect("timer should exist");
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(ms));
            if cancelled.load(Ordering::SeqCst) {
                return;
            }
            event_tx
                .send(PluginTimerEvent {
                    timer_id: id,
                    kind: PluginTimerKind::Timeout,
                })
                .ok();
        });
        id
    }

    pub(in crate::plugin) fn set_interval(&self, plugin: &str, ms: u64) -> u64 {
        let id = self.insert_timer(plugin, PluginTimerKind::Interval);
        let cancelled = self.cancelled_flag(id).expect("timer should exist");
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(ms));
                if cancelled.load(Ordering::SeqCst) {
                    break;
                }
                if event_tx
                    .send(PluginTimerEvent {
                        timer_id: id,
                        kind: PluginTimerKind::Interval,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        id
    }

    pub(in crate::plugin) fn clear(&self, timer_id: u64) -> bool {
        let Some(timer) = self
            .timers
            .lock()
            .expect("timer registry poisoned")
            .remove(&timer_id)
        else {
            return false;
        };
        timer.cancelled.store(true, Ordering::SeqCst);
        true
    }

    pub(in crate::plugin) fn poll_event(&self) -> Option<PluginTimerEvent> {
        self.event_rx
            .lock()
            .expect("timer event queue poisoned")
            .try_recv()
            .ok()
    }

    pub(in crate::plugin) fn mark_dispatched(&self, timer_id: u64) -> Option<String> {
        let mut timers = self.timers.lock().expect("timer registry poisoned");
        let timer = timers.get(&timer_id)?;
        if timer.cancelled.load(Ordering::SeqCst) {
            timers.remove(&timer_id);
            return None;
        }
        let plugin = timer.plugin.clone();
        if timer.kind != PluginTimerKind::Interval {
            timers.remove(&timer_id);
        }
        Some(plugin)
    }

    fn insert_timer(&self, plugin: &str, kind: PluginTimerKind) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.timers.lock().expect("timer registry poisoned").insert(
            id,
            PluginTimer {
                plugin: plugin.to_string(),
                kind,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
        id
    }

    fn cancelled_flag(&self, timer_id: u64) -> Option<Arc<AtomicBool>> {
        self.timers
            .lock()
            .expect("timer registry poisoned")
            .get(&timer_id)
            .map(|timer| Arc::clone(&timer.cancelled))
    }
}

pub(in crate::plugin) fn timer_id_from_number(value: f64) -> Result<u64, String> {
    BearNumber::new(value, "timer id")
        .non_negative_u64()
        .map_err(|_| format!("timer id must be a non-negative integer, got {value}"))
}

pub(in crate::plugin) fn timer_ms_from_number(value: f64) -> Result<u64, String> {
    BearNumber::new(value, "timer delay")
        .non_negative_u64()
        .map_err(|_| format!("timer delay must be a non-negative integer, got {value}"))
}
