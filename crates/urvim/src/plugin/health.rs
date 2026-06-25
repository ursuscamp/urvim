use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginHealth {
    pub(in crate::plugin) loaded: bool,
    pub(in crate::plugin) last_error: Option<String>,
    pub(in crate::plugin) slow_callback_count: u64,
    pub(in crate::plugin) timing: PluginTimingStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginTimingStats {
    pub(in crate::plugin) callback_count: u64,
    pub(in crate::plugin) total: Duration,
    pub(in crate::plugin) max: Duration,
    pub(in crate::plugin) last: Duration,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginHealthSummary {
    pub(in crate::plugin) loaded_count: usize,
    pub(in crate::plugin) failed_count: usize,
    pub(in crate::plugin) slow_callback_count: u64,
    pub(in crate::plugin) callback_count: u64,
    pub(in crate::plugin) max_callback: Duration,
}

impl PluginTimingStats {
    pub(in crate::plugin) fn record(&mut self, duration: Duration) {
        self.callback_count += 1;
        self.total += duration;
        self.max = self.max.max(duration);
        self.last = duration;
    }
}

pub(in crate::plugin) fn slow_threshold(duration: Duration) -> Duration {
    if duration >= Duration::from_millis(100) {
        Duration::from_millis(100)
    } else if duration >= Duration::from_millis(50) {
        Duration::from_millis(50)
    } else {
        Duration::from_millis(16)
    }
}
