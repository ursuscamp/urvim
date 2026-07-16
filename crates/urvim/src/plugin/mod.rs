use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::io;
use std::rc::Rc;
use std::time::{Duration, Instant};

mod callbacks;
mod confirmations;
mod conversion;
mod event;
mod fs;
mod health;
mod host;
mod jobs;
mod pickers;
mod timers;

use crate::actions::{execute_action_intent, execute_command_intent};
use bearscript::{Engine, Value};
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use urvim_core::buffer::{BufferId, Cursor, SyntaxSpan, TextRef};
use urvim_core::event::EditorEvent;
use urvim_core::globals;
use urvim_core::layout::Layout;
use urvim_core::ui::confirmation_box::{PluginConfirmationCancelled, PluginConfirmationSelection};
use urvim_core::ui::picker::plugin::PluginPickerCancelled;
use urvim_core::ui::{Command, Intent};

use callbacks::{BearscriptPlugin, BearscriptPluginCallbacks};
use confirmations::PluginConfirmationEvents;
use conversion::{BearNumber, BearValueRef, FromBearValue};
use event::{bear_args, event_constants, event_payload};
use fs::{PluginFsEvent, PluginFsRegistry, fs_event_id, fs_event_to_value};
use health::{PluginHealth, PluginHealthSummary, slow_threshold};
use host::{native_fn, urvim_module};
use jobs::{PluginJobEvent, PluginJobRegistry, job_event_to_value};
use pickers::PluginPickerEvents;
use timers::{PluginTimerEvent, PluginTimerKind, PluginTimerRegistry};

pub(super) type SharedLayout = Rc<RefCell<Layout>>;

/// In-process BearScript plugin runtime.
pub(super) struct BearscriptPluginRuntime {
    layout: SharedLayout,
    plugins: HashMap<String, BearscriptPlugin>,
    health: HashMap<String, PluginHealth>,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    fs: Rc<PluginFsRegistry>,
    jobs: Rc<PluginJobRegistry>,
    timers: Rc<PluginTimerRegistry>,
    picker_events: PluginPickerEvents,
    confirmation_events: PluginConfirmationEvents,
}

impl BearscriptPluginRuntime {
    /// Creates an empty runtime with no loaded plugins.
    #[cfg(test)]
    pub(super) fn empty(layout: SharedLayout) -> Self {
        Self {
            layout,
            plugins: HashMap::new(),
            health: HashMap::new(),
            contributions: Rc::new(RefCell::new(
                urvim_plugin::PluginContributionRegistry::default(),
            )),
            fs: Rc::new(PluginFsRegistry::default()),
            jobs: Rc::new(PluginJobRegistry::default()),
            timers: Rc::new(PluginTimerRegistry::default()),
            picker_events: PluginPickerEvents::default(),
            confirmation_events: PluginConfirmationEvents::default(),
        }
    }

    /// Loads all configured BearScript plugins and calls their `init()` hook.
    pub(super) fn load_from_registry(
        registry: &urvim_plugin::PluginRegistry,
        layout: SharedLayout,
    ) -> Self {
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let mut runtime = Self {
            layout,
            plugins: HashMap::new(),
            health: HashMap::new(),
            contributions,
            fs: Rc::new(PluginFsRegistry::default()),
            jobs: Rc::new(PluginJobRegistry::default()),
            timers: Rc::new(PluginTimerRegistry::default()),
            picker_events: PluginPickerEvents::default(),
            confirmation_events: PluginConfirmationEvents::default(),
        };

        for (plugin_name, plugin) in registry.iter() {
            if let Err(error) = runtime.load_plugin(plugin_name, plugin) {
                runtime.record_load_failure(plugin_name, error.clone());
                tracing::warn!(plugin = plugin_name, error = %error, "failed to load BearScript plugin");
                urvim_core::notify_warn!("Plugin {plugin_name} failed to load: {error}");
            }
        }

        runtime.apply_plugin_filetype_detection();
        runtime.publish_plugin_filetypes();

        runtime
    }

    fn publish_plugin_filetypes(&self) {
        urvim_core::globals::set_plugin_filetypes(self.contributions.borrow().filetype_names());
    }

    fn apply_plugin_filetype_detection(&self) {
        let contributions = self.contributions.borrow();
        globals::with_buffer_pool(|pool| {
            for buffer_id in pool.buffer_ids() {
                let Some(filetype) = pool.get(buffer_id).and_then(|buffer| {
                    buffer
                        .path()
                        .and_then(|path| path.extension())
                        .and_then(|extension| extension.to_str())
                        .and_then(|extension| contributions.filetype_for_extension(extension))
                        .map(str::to_string)
                }) else {
                    continue;
                };
                pool.with_buffer_mut(buffer_id, |buffer| {
                    if buffer.syntax_name() != filetype {
                        buffer.set_syntax_name(filetype);
                    }
                });
            }
        });
    }

    /// Returns a snapshot of runtime health for all configured plugins.
    pub(super) fn health_summary(&self) -> PluginHealthSummary {
        let mut summary = PluginHealthSummary::default();
        for health in self.health.values() {
            if health.loaded {
                summary.loaded_count += 1;
            } else {
                summary.failed_count += 1;
            }
            summary.slow_callback_count += health.slow_callback_count;
            summary.callback_count += health.timing.callback_count;
            summary.max_callback = summary.max_callback.max(health.timing.max);
        }
        summary
    }

    /// Formats plugin runtime health for the `plugin status` command.
    pub(super) fn status_summary(&self) -> String {
        let summary = self.health_summary();
        format!(
            "BearScript plugins: {} loaded, {} failed, {} callbacks, {} slow, slowest {}ms",
            summary.loaded_count,
            summary.failed_count,
            summary.callback_count,
            summary.slow_callback_count,
            summary.max_callback.as_millis()
        )
    }

    /// Runs a dynamically registered plugin command.
    pub(super) fn run_command(
        &mut self,
        plugin: &str,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        self.contributions
            .borrow()
            .command(plugin, command)
            .ok_or_else(|| format!("unknown plugin command {plugin} {command}"))?;

        let plugin_runtime = self
            .plugins
            .get_mut(plugin)
            .ok_or_else(|| format!("plugin {plugin:?} is not loaded"))?;
        let callback = plugin_runtime
            .callbacks
            .borrow()
            .commands
            .get(command)
            .cloned()
            .ok_or_else(|| format!("plugin {plugin:?} command {command:?} has no callback"))?;
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![bear_args(args)])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(plugin, format!("command {command}"), started.elapsed());
        if let Err(error) = &result {
            self.record_error(plugin, error.clone());
        }
        result
    }

    /// Dispatches an editor event to registered BearScript event hooks.
    pub(super) fn dispatch_editor_event(&mut self, event: EditorEvent) -> bool {
        let Some((kind, payload)) = event_payload(event) else {
            return false;
        };
        let targets: Vec<(String, u64)> = self
            .contributions
            .borrow()
            .event_hook_targets(kind)
            .map(|(plugin, hook_id)| (plugin.to_string(), hook_id))
            .collect();

        let mut dispatched = false;
        for (plugin, hook_id) in targets {
            let Some(plugin_runtime) = self.plugins.get_mut(&plugin) else {
                continue;
            };
            let Some(callback) = plugin_runtime
                .callbacks
                .borrow()
                .event_hooks
                .get(&hook_id)
                .cloned()
            else {
                continue;
            };
            let started = Instant::now();
            let result = plugin_runtime
                .engine
                .call_value(callback, vec![payload.clone()])
                .map(|_| ())
                .map_err(|error| error.to_string());
            self.record_callback(
                &plugin,
                format!("event {kind} hook {hook_id}"),
                started.elapsed(),
            );
            if let Err(error) = result {
                self.record_error(&plugin, error.clone());
                tracing::warn!(plugin, hook_id, error = %error, "BearScript event hook failed");
                urvim_core::notify_warn!("Plugin {plugin} event hook {hook_id} failed: {error}");
            }
            dispatched = true;
        }

        dispatched
    }

    /// Dispatches queued external job events to BearScript callbacks on the main thread.
    pub(super) fn dispatch_job_events(&mut self) -> bool {
        let mut dispatched = false;
        while let Some(event) = self.jobs.poll_event() {
            if self.dispatch_job_event(event) {
                dispatched = true;
            }
        }
        dispatched
    }

    /// Dispatches queued filesystem request completions to BearScript callbacks on the main thread.
    pub(super) fn dispatch_fs_events(&mut self) -> bool {
        let mut dispatched = false;
        while let Some(event) = self.fs.poll_event() {
            if self.dispatch_fs_event(event) {
                dispatched = true;
            }
        }
        dispatched
    }

    /// Runs synchronous syntax providers for buffers with matching plugin providers.
    pub(super) fn refresh_plugin_syntax(&mut self) -> bool {
        let visible_ranges = self.visible_ranges_by_buffer();
        let requested = self
            .plugins
            .values()
            .flat_map(|plugin| {
                plugin
                    .callbacks
                    .borrow()
                    .syntax_refresh_requests
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let targets = globals::with_buffer_pool(|pool| {
            pool.buffer_ids()
                .into_iter()
                .filter_map(|buffer_id| {
                    let buffer = pool.get(buffer_id)?;
                    let (plugin, provider_id) = {
                        let contributions = self.contributions.borrow();
                        let (plugin, provider) =
                            contributions.syntax_provider_for_filetype(buffer.syntax_name())?;
                        (plugin.to_string(), provider.id)
                    };
                    (requested.contains(&buffer_id) || !buffer.syntax_cache_complete()).then(|| {
                        (
                            buffer_id,
                            buffer.syntax_generation(),
                            buffer.syntax_name().to_string(),
                            buffer
                                .path()
                                .map(|path| path.to_string_lossy().into_owned()),
                            buffer.as_str(),
                            visible_ranges.get(&buffer_id).copied(),
                            plugin,
                            provider_id,
                        )
                    })
                })
                .collect::<Vec<_>>()
        });

        let mut refreshed = false;
        for (buffer_id, generation, filetype, path, text, visible_range, plugin, provider_id) in
            targets
        {
            if let Some(plugin_runtime) = self.plugins.get(&plugin) {
                plugin_runtime
                    .callbacks
                    .borrow_mut()
                    .syntax_refresh_requests
                    .remove(&buffer_id);
            }
            if self.run_syntax_provider(
                buffer_id,
                generation,
                filetype,
                path,
                text,
                visible_range,
                &plugin,
                provider_id,
            ) {
                refreshed = true;
            }
        }
        refreshed
    }

    fn run_syntax_provider(
        &mut self,
        buffer_id: BufferId,
        generation: u64,
        filetype: String,
        path: Option<String>,
        text: String,
        visible_range: Option<(usize, usize)>,
        plugin: &str,
        provider_id: u64,
    ) -> bool {
        let Some(plugin_runtime) = self.plugins.get_mut(plugin) else {
            return false;
        };
        let Some(callback) = plugin_runtime
            .callbacks
            .borrow()
            .syntax_providers
            .get(&provider_id)
            .cloned()
        else {
            return false;
        };
        let snapshot =
            syntax_snapshot_to_value(buffer_id, generation, &filetype, path, &text, visible_range);
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![snapshot])
            .map_err(|error| error.to_string())
            .and_then(|value| syntax_line_spans_from_value(&value, &text));
        self.record_callback(
            plugin,
            format!("syntax provider {provider_id}"),
            started.elapsed(),
        );

        match result {
            Ok(line_spans) => globals::with_buffer_mut(buffer_id, |buffer| {
                buffer.apply_external_syntax_spans(generation, line_spans)
            })
            .unwrap_or(false),
            Err(error) => {
                self.record_error(plugin, error.clone());
                tracing::warn!(plugin, provider_id, error = %error, "BearScript syntax provider failed");
                urvim_core::notify_warn!(
                    "Plugin {plugin} syntax provider {provider_id} failed: {error}"
                );
                globals::with_buffer_mut(buffer_id, |buffer| {
                    buffer.finish_external_syntax_refresh(generation)
                });
                false
            }
        }
    }

    fn visible_ranges_by_buffer(&self) -> HashMap<BufferId, (usize, usize)> {
        self.layout
            .borrow()
            .visible_range_snapshots(None)
            .into_iter()
            .map(|snapshot| (snapshot.buffer_id, (snapshot.start_line, snapshot.end_line)))
            .collect()
    }

    /// Dispatches queued timer callbacks on the main thread.
    pub(super) fn dispatch_timer_events(&mut self) -> bool {
        let mut dispatched = false;
        while let Some(event) = self.timers.poll_event() {
            if self.dispatch_timer_event(event) {
                dispatched = true;
            }
        }
        dispatched
    }

    /// Dispatches queued plugin picker cancellation callbacks on the main thread.
    pub(super) fn dispatch_picker_events(&mut self) -> bool {
        let mut dispatched = false;
        while let Some(event) = self.picker_events.poll() {
            dispatched |= self.dispatch_picker_cancellation(event);
        }
        dispatched
    }

    /// Dispatches queued plugin confirmation cancellation callbacks on the main thread.
    pub(super) fn dispatch_confirmation_events(&mut self) -> bool {
        let mut dispatched = false;
        while let Some(event) = self.confirmation_events.poll() {
            dispatched |= self.dispatch_confirmation_cancellation(event);
        }
        dispatched
    }

    /// Runs a plugin confirmation response callback.
    pub(super) fn run_confirmation_response(
        &mut self,
        plugin: &str,
        confirmation_id: u64,
        selection: PluginConfirmationSelection,
    ) -> Result<(), String> {
        let plugin_runtime = self
            .plugins
            .get_mut(plugin)
            .ok_or_else(|| format!("plugin {plugin:?} is not loaded"))?;
        let confirmation = plugin_runtime
            .callbacks
            .borrow_mut()
            .confirmations
            .remove(&confirmation_id)
            .ok_or_else(|| format!("plugin confirmation {confirmation_id} is not open"))?;
        let value = match selection {
            PluginConfirmationSelection::Primary => confirmation.primary_value,
            PluginConfirmationSelection::Secondary => confirmation.secondary_value,
        };
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(confirmation.on_response, vec![value])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(
            plugin,
            format!("confirmation {confirmation_id} response"),
            started.elapsed(),
        );
        if let Err(error) = &result {
            self.record_error(plugin, error.clone());
        }
        result
    }

    /// Runs a plugin picker selection callback.
    pub(super) fn run_picker_selection(
        &mut self,
        plugin: &str,
        picker_id: u64,
        item_id: u64,
    ) -> Result<(), String> {
        let plugin_runtime = self
            .plugins
            .get_mut(plugin)
            .ok_or_else(|| format!("plugin {plugin:?} is not loaded"))?;
        let picker = plugin_runtime
            .callbacks
            .borrow_mut()
            .pickers
            .remove(&picker_id)
            .ok_or_else(|| format!("plugin picker {picker_id} is not open"))?;
        let value = picker
            .values
            .get(&item_id)
            .cloned()
            .ok_or_else(|| format!("plugin picker item {item_id} is stale"))?;
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(picker.on_select, vec![value])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(
            plugin,
            format!("picker {picker_id} select"),
            started.elapsed(),
        );
        if let Err(error) = &result {
            self.record_error(plugin, error.clone());
        }
        result
    }

    fn dispatch_picker_cancellation(&mut self, event: PluginPickerCancelled) -> bool {
        let Some(plugin_runtime) = self.plugins.get_mut(&event.plugin) else {
            return false;
        };
        let Some(picker) = plugin_runtime
            .callbacks
            .borrow_mut()
            .pickers
            .remove(&event.picker_id)
        else {
            return false;
        };
        let Some(callback) = picker.on_cancel else {
            return true;
        };
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(
            &event.plugin,
            format!("picker {} cancel", event.picker_id),
            started.elapsed(),
        );
        if let Err(error) = result {
            self.record_error(&event.plugin, error.clone());
            tracing::warn!(plugin = event.plugin, picker_id = event.picker_id, error = %error, "BearScript picker callback failed");
            urvim_core::notify_warn!(
                "Plugin {} picker {} callback failed: {error}",
                event.plugin,
                event.picker_id
            );
        }
        true
    }

    fn dispatch_confirmation_cancellation(&mut self, event: PluginConfirmationCancelled) -> bool {
        let Some(plugin_runtime) = self.plugins.get_mut(&event.plugin) else {
            return false;
        };
        let Some(confirmation) = plugin_runtime
            .callbacks
            .borrow_mut()
            .confirmations
            .remove(&event.confirmation_id)
        else {
            return false;
        };
        let Some(callback) = confirmation.on_cancel else {
            return true;
        };
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(
            &event.plugin,
            format!("confirmation {} cancel", event.confirmation_id),
            started.elapsed(),
        );
        if let Err(error) = result {
            self.record_error(&event.plugin, error.clone());
            tracing::warn!(plugin = event.plugin, confirmation_id = event.confirmation_id, error = %error, "BearScript confirmation callback failed");
            urvim_core::notify_warn!(
                "Plugin {} confirmation {} callback failed: {error}",
                event.plugin,
                event.confirmation_id
            );
        }
        true
    }

    fn dispatch_fs_event(&mut self, event: PluginFsEvent) -> bool {
        let request_id = fs_event_id(&event);
        let Some(plugin) = self.fs.mark_finished(request_id) else {
            return false;
        };
        let Some(plugin_runtime) = self.plugins.get_mut(&plugin) else {
            return false;
        };
        let callback = plugin_runtime.callbacks.borrow_mut().fs.remove(&request_id);
        let Some(callback) = callback else {
            return false;
        };
        let payload = fs_event_to_value(&event);
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![payload])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(
            &plugin,
            format!("fs request {request_id}"),
            started.elapsed(),
        );
        if let Err(error) = result {
            self.record_error(&plugin, error.clone());
            tracing::warn!(plugin, request_id, error = %error, "BearScript filesystem callback failed");
            urvim_core::notify_warn!(
                "Plugin {plugin} filesystem request {request_id} callback failed: {error}"
            );
        }
        true
    }

    fn dispatch_timer_event(&mut self, event: PluginTimerEvent) -> bool {
        let Some(plugin) = self.timers.mark_dispatched(event.timer_id) else {
            return false;
        };
        let Some(plugin_runtime) = self.plugins.get_mut(&plugin) else {
            return false;
        };
        let callback = {
            let callbacks = plugin_runtime.callbacks.borrow();
            callbacks.timers.get(&event.timer_id).cloned()
        };
        if event.kind != PluginTimerKind::Interval {
            plugin_runtime
                .callbacks
                .borrow_mut()
                .timers
                .remove(&event.timer_id);
        }
        let Some(callback) = callback else {
            return false;
        };
        let label = format!("timer {} {}", event.timer_id, event.kind.as_str());
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(&plugin, label, started.elapsed());
        if let Err(error) = result {
            self.record_error(&plugin, error.clone());
            tracing::warn!(plugin, timer_id = event.timer_id, error = %error, "BearScript timer callback failed");
            urvim_core::notify_warn!(
                "Plugin {plugin} timer {} callback failed: {error}",
                event.timer_id
            );
        }
        true
    }

    fn dispatch_job_event(&mut self, event: PluginJobEvent) -> bool {
        let job_id = match &event {
            PluginJobEvent::Stdout { job_id, .. }
            | PluginJobEvent::Stderr { job_id, .. }
            | PluginJobEvent::Exit { job_id, .. } => *job_id,
        };
        let plugin = match &event {
            PluginJobEvent::Exit { status, .. } => self.jobs.mark_finished(job_id, *status),
            _ => self.jobs.plugin_for_job(job_id),
        };
        let Some(plugin) = plugin else {
            return false;
        };
        let Some(plugin_runtime) = self.plugins.get_mut(&plugin) else {
            return false;
        };
        let callback = {
            let callbacks = plugin_runtime.callbacks.borrow();
            let Some(job_callbacks) = callbacks.jobs.get(&job_id) else {
                return false;
            };
            match &event {
                PluginJobEvent::Stdout { .. } => job_callbacks.on_stdout.clone(),
                PluginJobEvent::Stderr { .. } => job_callbacks.on_stderr.clone(),
                PluginJobEvent::Exit { .. } => job_callbacks.on_exit.clone(),
            }
        };
        if matches!(event, PluginJobEvent::Exit { .. }) {
            plugin_runtime.callbacks.borrow_mut().jobs.remove(&job_id);
        }
        let Some(callback) = callback else {
            return false;
        };
        let label = match &event {
            PluginJobEvent::Stdout { .. } => format!("job {job_id} stdout"),
            PluginJobEvent::Stderr { .. } => format!("job {job_id} stderr"),
            PluginJobEvent::Exit { .. } => format!("job {job_id} exit"),
        };
        let payload = job_event_to_value(&event);
        let started = Instant::now();
        let result = plugin_runtime
            .engine
            .call_value(callback, vec![payload])
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.record_callback(&plugin, label, started.elapsed());
        if let Err(error) = result {
            self.record_error(&plugin, error.clone());
            tracing::warn!(plugin, job_id, error = %error, "BearScript job callback failed");
            urvim_core::notify_warn!("Plugin {plugin} job {job_id} callback failed: {error}");
        }
        true
    }

    fn load_plugin(
        &mut self,
        plugin_name: &str,
        plugin: &urvim_plugin::LoadedPlugin,
    ) -> Result<(), String> {
        let mut engine = Engine::new();
        engine.set_current_dir(plugin.root().to_path_buf());
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        callbacks.borrow_mut().picker_cancellation_sender = Some(self.picker_events.sender());
        callbacks.borrow_mut().confirmation_cancellation_sender =
            Some(self.confirmation_events.sender());
        engine.set_global(
            "urvim",
            urvim_module(
                plugin_name.to_string(),
                Rc::clone(&self.contributions),
                Rc::clone(&callbacks),
                Rc::clone(&self.layout),
                Rc::clone(&self.fs),
                Rc::clone(&self.jobs),
                Rc::clone(&self.timers),
            ),
        );

        let entry = plugin.root().join(plugin.entry());
        if let Err(error) = engine.eval_file(entry.to_string_lossy().as_ref()) {
            self.layout
                .borrow_mut()
                .plugin_windows_mut()
                .close_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_panes_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_picker_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_confirmation_owned(plugin_name);
            return Err(error.to_string());
        }
        let started = Instant::now();
        let init_result = engine.eval("init();").map_err(|error| error.to_string());
        self.record_callback(plugin_name, "init", started.elapsed());
        if let Err(error) = init_result {
            self.layout
                .borrow_mut()
                .plugin_windows_mut()
                .close_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_panes_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_picker_owned(plugin_name);
            self.layout
                .borrow_mut()
                .close_plugin_confirmation_owned(plugin_name);
            return Err(error);
        }

        self.plugins.insert(
            plugin_name.to_string(),
            BearscriptPlugin { engine, callbacks },
        );
        self.health
            .entry(plugin_name.to_string())
            .and_modify(|health| health.loaded = true)
            .or_insert_with(|| PluginHealth {
                loaded: true,
                ..PluginHealth::default()
            });
        Ok(())
    }

    fn record_load_failure(&mut self, plugin: &str, error: String) {
        self.health.insert(
            plugin.to_string(),
            PluginHealth {
                loaded: false,
                last_error: Some(error),
                ..PluginHealth::default()
            },
        );
    }

    fn record_error(&mut self, plugin: &str, error: String) {
        self.health
            .entry(plugin.to_string())
            .or_default()
            .last_error = Some(error);
    }

    fn record_callback(&mut self, plugin: &str, callback: impl AsRef<str>, duration: Duration) {
        let callback = callback.as_ref();
        let health = self.health.entry(plugin.to_string()).or_default();
        health.timing.record(duration);
        if duration >= Duration::from_millis(16) {
            health.slow_callback_count += 1;
            let threshold = slow_threshold(duration);
            tracing::warn!(
                plugin,
                callback,
                elapsed_ms = duration.as_millis(),
                threshold_ms = threshold.as_millis(),
                "slow BearScript plugin callback"
            );
            if threshold >= Duration::from_millis(50) {
                urvim_core::notify_warn!(
                    "Plugin {plugin} {callback} took {}ms",
                    duration.as_millis()
                );
            }
        }
    }
}

fn buffers_module() -> Value {
    Value::Module(HashMap::from([
        ("active".to_string(), buffers_module_active_fn()),
        ("list".to_string(), buffers_module_list_fn()),
        (
            "exists".to_string(),
            native_fn("buffers.exists", |buffer_id: Value| {
                let buffer_id = buffer_id_from_value(&buffer_id)?;
                Ok(globals::with_buffer(buffer_id, |_| ()).is_some())
            }),
        ),
        (
            "name".to_string(),
            native_fn("buffers.name", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |buffer_id, buffer| {
                    Ok(buffer
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| format!("Untitled {}", buffer_id.get())))
                })
            }),
        ),
        (
            "path".to_string(),
            native_fn("buffers.path", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |_buffer_id, buffer| {
                    Ok(buffer
                        .path()
                        .map(|path| Value::String(path.to_string_lossy().into_owned().into()))
                        .unwrap_or(Value::Null))
                })
            }),
        ),
        (
            "filetype".to_string(),
            native_fn("buffers.filetype", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |_buffer_id, buffer| {
                    Ok(buffer.syntax_name().to_string())
                })
            }),
        ),
        (
            "set_filetype".to_string(),
            native_fn(
                "buffers.set_filetype",
                |buffer_id: Value, filetype: String| {
                    urvim_plugin::validate_contribution_name(&filetype, "filetype")?;
                    let canonical = urvim_core::buffer::resolve_builtin_syntax_label(&filetype)
                        .unwrap_or(filetype);
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    let changed = globals::with_buffer_mut(buffer_id, |buffer| {
                        let changed = buffer.syntax_name() != canonical;
                        buffer.set_syntax_name(canonical);
                        changed
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))?;
                    if changed {
                        globals::enqueue_editor_event(EditorEvent::BufferFiletypeChanged {
                            buffer_id,
                        });
                    }
                    Ok(())
                },
            ),
        ),
        (
            "is_modified".to_string(),
            native_fn("buffers.is_modified", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |_buffer_id, buffer| Ok(buffer.is_modified()))
            }),
        ),
        (
            "line_count".to_string(),
            native_fn("buffers.line_count", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |_buffer_id, buffer| {
                    Ok(buffer.line_count() as f64)
                })
            }),
        ),
        (
            "line".to_string(),
            native_fn("buffers.line", |buffer_id: Value, row: Value| {
                let row = usize_from_value(&row, "row")?;
                with_existing_buffer(buffer_id, |buffer_id, buffer| {
                    buffer
                        .line_at(row)
                        .map(|line| line.to_text())
                        .ok_or_else(|| row_out_of_range_error(buffer_id, row))
                })
            }),
        ),
        (
            "lines".to_string(),
            native_fn(
                "buffers.lines",
                |buffer_id: Value, start_row: Value, end_row: Value| {
                    let start_row = usize_from_value(&start_row, "start_row")?;
                    let end_row = usize_from_value(&end_row, "end_row")?;
                    if start_row > end_row {
                        return Err(format!(
                            "start_row must be less than or equal to end_row, got {start_row} > {end_row}"
                        ));
                    }
                    with_existing_buffer(buffer_id, |buffer_id, buffer| {
                        if end_row > buffer.line_count() {
                            return Err(row_out_of_range_error(buffer_id, end_row));
                        }
                        let lines = (start_row..end_row)
                            .map(|row| {
                                buffer
                                    .line_at(row)
                                    .map(|line| {
                                        Value::String(line.to_text().into_boxed_str().into())
                                    })
                                    .ok_or_else(|| row_out_of_range_error(buffer_id, row))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(Value::List(lines.into()))
                    })
                },
            ),
        ),
        (
            "text".to_string(),
            native_fn("buffers.text", |buffer_id: Value| {
                with_existing_buffer(buffer_id, |_buffer_id, buffer| Ok(buffer.as_str()))
            }),
        ),
        (
            "set_line".to_string(),
            native_fn(
                "buffers.set_line",
                |buffer_id: Value, row: Value, text: String| {
                    if text.contains('\n') {
                        return Err("line text must not contain newlines".to_string());
                    }
                    let row = usize_from_value(&row, "row")?;
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    with_existing_buffer_mut(buffer_id, |buffer_id, buffer| {
                        let line_len = line_len_or_row_error(buffer_id, buffer, row)?;
                        buffer.apply_text_edits(&[(
                            Cursor::new(row, 0),
                            Cursor::new(row, line_len),
                            text,
                        )]);
                        Ok(())
                    })
                },
            ),
        ),
        (
            "insert_line".to_string(),
            native_fn(
                "buffers.insert_line",
                |buffer_id: Value, row: Value, text: String| {
                    if text.contains('\n') {
                        return Err("line text must not contain newlines".to_string());
                    }
                    let row = usize_from_value(&row, "row")?;
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    with_existing_buffer_mut(buffer_id, |buffer_id, buffer| {
                        if row > buffer.line_count() {
                            return Err(row_out_of_range_error(buffer_id, row));
                        }
                        let insert = if row == buffer.line_count() {
                            format!("\n{text}")
                        } else {
                            format!("{text}\n")
                        };
                        let cursor = if row == buffer.line_count() {
                            let last_row = buffer.line_count().saturating_sub(1);
                            Cursor::new(last_row, buffer.line_len(last_row))
                        } else {
                            Cursor::new(row, 0)
                        };
                        buffer.insert_text(cursor, &insert);
                        buffer.push_snapshot(buffer.current_cursor());
                        Ok(())
                    })
                },
            ),
        ),
        (
            "delete_line".to_string(),
            native_fn("buffers.delete_line", |buffer_id: Value, row: Value| {
                let row = usize_from_value(&row, "row")?;
                let buffer_id = buffer_id_from_value(&buffer_id)?;
                with_existing_buffer_mut(buffer_id, |buffer_id, buffer| {
                    line_len_or_row_error(buffer_id, buffer, row)?;
                    if buffer.line_count() == 1 {
                        let line_len = buffer.line_len(0);
                        buffer.apply_text_edits(&[(
                            Cursor::new(0, 0),
                            Cursor::new(0, line_len),
                            String::new(),
                        )]);
                    } else if row + 1 == buffer.line_count() {
                        let previous = row - 1;
                        let previous_len = buffer.line_len(previous);
                        let line_len = buffer.line_len(row);
                        buffer.apply_text_edits(&[(
                            Cursor::new(previous, previous_len),
                            Cursor::new(row, line_len),
                            String::new(),
                        )]);
                    } else {
                        buffer.apply_text_edits(&[(
                            Cursor::new(row, 0),
                            Cursor::new(row + 1, 0),
                            String::new(),
                        )]);
                    }
                    Ok(())
                })
            }),
        ),
        (
            "replace_range".to_string(),
            native_fn(
                "buffers.replace_range",
                |buffer_id: Value, range: Value, text: String| {
                    let text_range = range_from_value(&range)?;
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    with_existing_buffer_mut(buffer_id, |buffer_id, buffer| {
                        ensure_valid_cursor(buffer_id, buffer, text_range.start, "range.start")?;
                        ensure_valid_cursor(buffer_id, buffer, text_range.end, "range.end")?;
                        if text_range.start > text_range.end {
                            return Err(
                                "range start must be before or equal to range end".to_string()
                            );
                        }
                        buffer.apply_text_edits(&[(text_range.start, text_range.end, text)]);
                        Ok(())
                    })
                },
            ),
        ),
        (
            "save".to_string(),
            native_fn("buffers.save", |buffer_id: Value| {
                let buffer_id = buffer_id_from_value(&buffer_id)?;
                save_buffer_for_plugin(buffer_id)
            }),
        ),
    ]).into())
}

fn buffers_module_active_fn() -> Value {
    native_fn("buffers.active", || {
        Ok(globals::with_active_buffer_id(|id| {
            id.map(|id| Value::Number(id.get() as f64))
                .unwrap_or(Value::Null)
        }))
    })
}

fn buffers_module_list_fn() -> Value {
    native_fn("buffers.list", || {
        let buffers = globals::with_buffer_pool(|pool| {
            pool.buffer_ids()
                .into_iter()
                .map(|id| Value::Number(id.get() as f64))
                .collect::<Vec<_>>()
        });
        Ok(Value::List(buffers.into()))
    })
}

fn windows_module(layout: SharedLayout) -> Value {
    let active_layout = Rc::clone(&layout);
    let list_layout = Rc::clone(&layout);
    let buffer_layout = Rc::clone(&layout);
    let cursor_layout = Rc::clone(&layout);
    let set_cursor_layout = Rc::clone(&layout);
    let visible_range_layout = Rc::clone(&layout);
    let open_buffer_layout = Rc::clone(&layout);
    Value::Module(
        HashMap::from([
            (
                "active".to_string(),
                native_fn("windows.active", move || {
                    Ok(active_layout
                        .borrow()
                        .active_window_id()
                        .map(|id| Value::Number(id.0 as f64))
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "list".to_string(),
                native_fn("windows.list", move || {
                    Ok(Value::List(
                        list_layout
                            .borrow()
                            .window_ids()
                            .into_iter()
                            .map(|id| Value::Number(id.0 as f64))
                            .collect::<Vec<_>>()
                            .into(),
                    ))
                }),
            ),
            (
                "buffer".to_string(),
                native_fn("windows.buffer", move |window_id: Value| {
                    let window_id = window_id_from_value(&window_id)?;
                    let layout = buffer_layout.borrow();
                    let view = layout
                        .buffer_view_for_window(window_id)
                        .ok_or_else(|| unknown_window_error(window_id))?;
                    Ok(view.buffer_id().get() as f64)
                }),
            ),
            (
                "cursor".to_string(),
                native_fn("windows.cursor", move |window_id: Value| {
                    let window_id = window_id_from_value(&window_id)?;
                    let layout = cursor_layout.borrow();
                    let view = layout
                        .buffer_view_for_window(window_id)
                        .ok_or_else(|| unknown_window_error(window_id))?;
                    Ok(cursor_to_value(view.cursor()))
                }),
            ),
            (
                "set_cursor".to_string(),
                native_fn(
                    "windows.set_cursor",
                    move |window_id: Value, row: Value, col: Value| {
                        let window_id = window_id_from_value(&window_id)?;
                        let cursor = Cursor::new(
                            usize_from_value(&row, "row")?,
                            usize_from_value(&col, "col")?,
                        );
                        let mut layout = set_cursor_layout.borrow_mut();
                        let view = layout
                            .buffer_view_for_window_mut(window_id)
                            .ok_or_else(|| unknown_window_error(window_id))?;
                        let buffer_id = view.buffer_id();
                        globals::with_buffer(buffer_id, |buffer| {
                            ensure_valid_cursor(buffer_id, buffer, cursor, "cursor")
                        })
                        .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                        view.set_cursor(cursor);
                        Ok(())
                    },
                ),
            ),
            (
                "visible_range".to_string(),
                native_fn("windows.visible_range", move |window_id: Value| {
                    let window_id = window_id_from_value(&window_id)?;
                    let layout = visible_range_layout.borrow();
                    let view = layout
                        .buffer_view_for_window(window_id)
                        .ok_or_else(|| unknown_window_error(window_id))?;
                    let start_row = view.scroll_offset().row as usize;
                    let buffer_id = view.buffer_id();
                    let line_count = globals::with_buffer(buffer_id, |buffer| buffer.line_count())
                        .ok_or_else(|| unknown_buffer_error(buffer_id))?;
                    let height = layout
                        .pane_region(window_id)
                        .map(|region| region.size.rows)
                        .unwrap_or_else(|| layout.size().rows.saturating_sub(1))
                        as usize;
                    let end_row = start_row.saturating_add(height).min(line_count);
                    Ok(row_range_to_value(start_row, end_row))
                }),
            ),
            (
                "open_buffer".to_string(),
                native_fn("windows.open_buffer", move |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    if globals::with_buffer(buffer_id, |_| ()).is_none() {
                        return Err(unknown_buffer_error(buffer_id));
                    }
                    open_buffer_layout
                        .borrow_mut()
                        .window_group_mut()
                        .activate_or_open_buffer(buffer_id);
                    globals::set_active_buffer_id(buffer_id);
                    Ok(())
                }),
            ),
        ])
        .into(),
    )
}

fn selection_module(layout: SharedLayout) -> Value {
    let get_layout = Rc::clone(&layout);
    let text_layout = Rc::clone(&layout);
    let set_layout = Rc::clone(&layout);
    let clear_layout = Rc::clone(&layout);
    let replace_layout = Rc::clone(&layout);
    Value::Module(
        HashMap::from([
            (
                "get".to_string(),
                native_fn("selection.get", move || {
                    let layout = get_layout.borrow();
                    let view = layout.active_buffer_view();
                    Ok(selection_range_for_view(view)
                        .map(range_to_value)
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "text".to_string(),
                native_fn("selection.text", move || {
                    let layout = text_layout.borrow();
                    let view = layout.active_buffer_view();
                    let Some(range) = selection_range_for_view(view) else {
                        return Ok(Value::Null);
                    };
                    let buffer_id = view.buffer_id();
                    let text = globals::with_buffer(buffer_id, |buffer| {
                        buffer.text_in_range(range.start, range.end)
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))?
                    .ok_or_else(|| "selection range is out of range".to_string())?;
                    Ok(Value::String(text.into_boxed_str().into()))
                }),
            ),
            (
                "set".to_string(),
                native_fn("selection.set", move |range: Value| {
                    let range = range_from_value(&range)?;
                    let mut layout = set_layout.borrow_mut();
                    let view = layout.active_buffer_view_mut();
                    let buffer_id = view.buffer_id();
                    globals::with_buffer(buffer_id, |buffer| {
                        ensure_valid_cursor(buffer_id, buffer, range.start, "range.start")?;
                        ensure_valid_cursor(buffer_id, buffer, range.end, "range.end")?;
                        Ok::<(), String>(())
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                    if range.start > range.end {
                        return Err("range start must be before or equal to range end".to_string());
                    }
                    view.set_cursor(range.start);
                    view.begin_visual_selection(urvim_core::window::VisualSelectionKind::Character);
                    view.set_visual_selection_range(urvim_core::buffer::TextObjectRange {
                        start: range.start,
                        end: range.end,
                    });
                    Ok(())
                }),
            ),
            (
                "clear".to_string(),
                native_fn("selection.clear", move || {
                    clear_layout
                        .borrow_mut()
                        .active_buffer_view_mut()
                        .clear_visual_selection();
                    Ok(())
                }),
            ),
            (
                "replace".to_string(),
                native_fn("selection.replace", move |text: String| {
                    let mut layout = replace_layout.borrow_mut();
                    let view = layout.active_buffer_view_mut();
                    let Some(range) = selection_range_for_view(view) else {
                        return Err("no active selection".to_string());
                    };
                    let buffer_id = view.buffer_id();
                    globals::with_buffer_mut(buffer_id, |buffer| {
                        buffer.apply_text_edits(&[(range.start, range.end, text)]);
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))?;
                    view.set_cursor(range.start);
                    view.clear_visual_selection();
                    Ok(())
                }),
            ),
        ])
        .into(),
    )
}

fn diagnostics_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "set".to_string(),
                native_fn(
                    "diagnostics.set",
                    |namespace: String, buffer_id: Value, diagnostics: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        ensure_buffer_exists(buffer_id)?;
                        let diagnostics = diagnostics_from_value(&diagnostics, buffer_id)?;
                        globals::with_diagnostics_store(|store| {
                            store.set(buffer_id, namespace, diagnostics);
                        })
                        .ok_or_else(|| "diagnostics store is unavailable".to_string())?;
                        globals::enqueue_editor_event(EditorEvent::DiagnosticsChanged {
                            buffer_id,
                        });
                        Ok(())
                    },
                ),
            ),
            (
                "clear".to_string(),
                native_fn(
                    "diagnostics.clear",
                    |namespace: String, buffer_id: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        ensure_buffer_exists(buffer_id)?;
                        globals::with_diagnostics_store(|store| {
                            store.clear(buffer_id, &namespace);
                        })
                        .ok_or_else(|| "diagnostics store is unavailable".to_string())?;
                        globals::enqueue_editor_event(EditorEvent::DiagnosticsChanged {
                            buffer_id,
                        });
                        Ok(())
                    },
                ),
            ),
            (
                "get".to_string(),
                native_fn(
                    "diagnostics.get",
                    |buffer_id: Value, namespace: Option<String>| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        ensure_buffer_exists(buffer_id)?;
                        let diagnostics =
                            globals::with_diagnostics_store(|store| match namespace.as_deref() {
                                Some(namespace) => {
                                    store.diagnostics_for_buffer_source(buffer_id, namespace)
                                }
                                None => store.diagnostics_for_buffer(buffer_id),
                            })
                            .ok_or_else(|| "diagnostics store is unavailable".to_string())?;
                        Ok(Value::List(
                            diagnostics
                                .iter()
                                .map(diagnostic_to_value)
                                .collect::<Vec<_>>()
                                .into(),
                        ))
                    },
                ),
            ),
            (
                "counts".to_string(),
                native_fn("diagnostics.counts", |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    ensure_buffer_exists(buffer_id)?;
                    let counts = globals::with_diagnostics_store(|store| {
                        store.diagnostic_counts_for_buffer(buffer_id)
                    })
                    .ok_or_else(|| "diagnostics store is unavailable".to_string())?;
                    Ok(Value::Map(
                        HashMap::from([
                            ("error".to_string(), Value::Number(counts.error as f64)),
                            ("warning".to_string(), Value::Number(counts.warning as f64)),
                            ("info".to_string(), Value::Number(counts.info as f64)),
                            ("hint".to_string(), Value::Number(counts.hint as f64)),
                        ])
                        .into(),
                    ))
                }),
            ),
        ])
        .into(),
    )
}

fn register_plugin_command(
    plugin: &str,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    name: String,
    callback: Value,
    description: Option<String>,
) -> Result<(), String> {
    validate_callback(&callback, "command callback")?;
    contributions.borrow_mut().register_command(
        plugin.to_string(),
        urvim_plugin::DynamicPluginCommand {
            name: name.clone(),
            description,
        },
    )?;
    callbacks.borrow_mut().commands.insert(name, callback);
    Ok(())
}

fn unregister_plugin_command(
    plugin: &str,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    name: &str,
) {
    contributions.borrow_mut().unregister_command(plugin, name);
    callbacks.borrow_mut().commands.remove(name);
}

fn command_to_value(command: &urvim_plugin::DynamicPluginCommand) -> Value {
    Value::Map(
        HashMap::from([
            (
                "name".to_string(),
                Value::String(command.name.clone().into_boxed_str().into()),
            ),
            (
                "description".to_string(),
                command
                    .description
                    .clone()
                    .map(|description| Value::String(description.into_boxed_str().into()))
                    .unwrap_or(Value::Null),
            ),
        ])
        .into(),
    )
}

fn command_execute_fn(name: &str, layout: SharedLayout) -> Value {
    native_fn(name, move |command_line: String| {
        execute_command_line_for_plugin(Rc::clone(&layout), &command_line)
    })
}

fn execute_command_line_for_plugin(
    layout: SharedLayout,
    command_line: &str,
) -> Result<bool, String> {
    let intents =
        urvim_core::command::parse_many(command_line).map_err(|error| error.to_string())?;
    for intent in &intents {
        validate_plugin_command_execution_intent(intent)?;
    }

    let mut handled_all = true;
    for intent in intents {
        handled_all &= match intent {
            Intent::Editor(action) => execute_action_intent(&mut layout.borrow_mut(), action),
            Intent::Command(command) => {
                execute_command_intent(&mut layout.borrow_mut(), None, command)
            }
        };
    }
    Ok(handled_all)
}

pub(in crate::plugin) fn validate_plugin_command_execution_intent(
    intent: &Intent,
) -> Result<(), String> {
    let Intent::Command(command) = intent else {
        return Ok(());
    };

    match command {
        Command::PluginRequest { .. } | Command::PluginStatus => {
            Err("urvim.command does not allow plugin commands".to_string())
        }
        Command::Quit | Command::TryQuit => {
            Err("urvim.command does not allow quit commands".to_string())
        }
        Command::OverwriteBuffer(_) => {
            Err("urvim.command does not allow overwrite confirmation commands".to_string())
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy)]
struct ScriptRange {
    start: Cursor,
    end: Cursor,
}

fn buffer_id_from_value(value: &Value) -> Result<BufferId, String> {
    Ok(BufferId::new(usize_from_value(value, "buffer_id")?))
}

fn window_id_from_value(value: &Value) -> Result<urvim_core::layout::PaneId, String> {
    Ok(urvim_core::layout::PaneId(usize_from_value(
        value,
        "window_id",
    )?))
}

fn usize_from_value(value: &Value, label: &str) -> Result<usize, String> {
    usize::from_bear(BearValueRef::new(value, label))
        .map_err(|_| format!("{label} must be a non-negative integer"))
}

fn with_existing_buffer<R>(
    buffer_id: Value,
    f: impl FnOnce(BufferId, &urvim_core::buffer::Buffer) -> Result<R, String>,
) -> Result<R, String> {
    let buffer_id = buffer_id_from_value(&buffer_id)?;
    globals::with_buffer(buffer_id, |buffer| f(buffer_id, buffer))
        .ok_or_else(|| unknown_buffer_error(buffer_id))?
}

fn with_existing_buffer_mut<R>(
    buffer_id: BufferId,
    f: impl FnOnce(BufferId, &mut urvim_core::buffer::Buffer) -> Result<R, String>,
) -> Result<R, String> {
    globals::with_buffer_mut(buffer_id, |buffer| f(buffer_id, buffer))
        .ok_or_else(|| unknown_buffer_error(buffer_id))?
}

fn unknown_buffer_error(buffer_id: BufferId) -> String {
    format!("unknown buffer_id {}", buffer_id.get())
}

fn unknown_window_error(window_id: urvim_core::layout::PaneId) -> String {
    format!("unknown window_id {}", window_id.0)
}

fn cursor_to_value(cursor: Cursor) -> Value {
    Value::Map(
        HashMap::from([
            ("row".to_string(), Value::Number(cursor.line as f64)),
            ("col".to_string(), Value::Number(cursor.col as f64)),
        ])
        .into(),
    )
}

fn row_range_to_value(start_row: usize, end_row: usize) -> Value {
    Value::Map(
        HashMap::from([
            ("start_row".to_string(), Value::Number(start_row as f64)),
            ("end_row".to_string(), Value::Number(end_row as f64)),
        ])
        .into(),
    )
}

fn range_to_value(range: ScriptRange) -> Value {
    Value::Map(
        HashMap::from([
            ("start".to_string(), cursor_to_value(range.start)),
            ("end".to_string(), cursor_to_value(range.end)),
        ])
        .into(),
    )
}

fn selection_range_for_view(view: &urvim_core::window::BufferView) -> Option<ScriptRange> {
    view.visual_selection_range().map(|range| ScriptRange {
        start: range.start,
        end: range.end,
    })
}

fn row_out_of_range_error(buffer_id: BufferId, row: usize) -> String {
    format!(
        "row {row} is out of range for buffer_id {}",
        buffer_id.get()
    )
}

fn line_len_or_row_error(
    buffer_id: BufferId,
    buffer: &urvim_core::buffer::Buffer,
    row: usize,
) -> Result<usize, String> {
    if row >= buffer.line_count() {
        return Err(row_out_of_range_error(buffer_id, row));
    }
    Ok(buffer.line_len(row))
}

fn ensure_valid_cursor(
    buffer_id: BufferId,
    buffer: &urvim_core::buffer::Buffer,
    cursor: Cursor,
    label: &str,
) -> Result<(), String> {
    if buffer.is_valid_cursor(cursor) {
        Ok(())
    } else {
        Err(format!(
            "{label} row {} col {} is out of range for buffer_id {}",
            cursor.line,
            cursor.col,
            buffer_id.get()
        ))
    }
}

fn ensure_buffer_exists(buffer_id: BufferId) -> Result<(), String> {
    globals::with_buffer(buffer_id, |_| ())
        .ok_or_else(|| unknown_buffer_error(buffer_id))
        .map(|_| ())
}

fn diagnostics_from_value(value: &Value, buffer_id: BufferId) -> Result<Vec<Diagnostic>, String> {
    let Value::List(values) = value else {
        return Err("diagnostics must be a list".to_string());
    };
    values
        .iter()
        .map(|value| diagnostic_from_value(value, buffer_id))
        .collect()
}

fn diagnostic_from_value(value: &Value, buffer_id: BufferId) -> Result<Diagnostic, String> {
    let Value::Map(map) = value else {
        return Err("diagnostic must be a map".to_string());
    };
    let range = range_from_value(
        map.get("range")
            .ok_or_else(|| "diagnostic requires range".to_string())?,
    )?;
    with_existing_buffer(
        Value::Number(buffer_id.get() as f64),
        |buffer_id, buffer| {
            ensure_valid_cursor(buffer_id, buffer, range.start, "range.start")?;
            ensure_valid_cursor(buffer_id, buffer, range.end, "range.end")?;
            Ok(())
        },
    )?;
    if range.start > range.end {
        return Err("range start must be before or equal to range end".to_string());
    }
    let severity = string_field(map, "severity")?;
    let message = string_field(map, "message")?;
    let source = optional_string_field(map, "source")?;
    Ok(Diagnostic {
        range: Range::new(
            position_from_cursor(range.start),
            position_from_cursor(range.end),
        ),
        severity: Some(severity_from_string(&severity)?),
        code: None,
        code_description: None,
        source,
        message,
        related_information: None,
        tags: None,
        data: None,
    })
}

fn diagnostic_to_value(diagnostic: &Diagnostic) -> Value {
    Value::Map(
        HashMap::from([
            (
                "range".to_string(),
                range_to_value(ScriptRange {
                    start: cursor_from_position(diagnostic.range.start),
                    end: cursor_from_position(diagnostic.range.end),
                }),
            ),
            (
                "severity".to_string(),
                Value::String(severity_to_string(diagnostic.severity).into()),
            ),
            (
                "message".to_string(),
                Value::String(diagnostic.message.clone().into_boxed_str().into()),
            ),
            (
                "source".to_string(),
                diagnostic
                    .source
                    .clone()
                    .map(|source| Value::String(source.into_boxed_str().into()))
                    .unwrap_or(Value::Null),
            ),
        ])
        .into(),
    )
}

fn position_from_cursor(cursor: Cursor) -> Position {
    Position::new(cursor.line as u32, cursor.col as u32)
}

fn cursor_from_position(position: Position) -> Cursor {
    Cursor::new(position.line as usize, position.character as usize)
}

fn severity_from_string(severity: &str) -> Result<DiagnosticSeverity, String> {
    match severity {
        "error" => Ok(DiagnosticSeverity::ERROR),
        "warning" | "warn" => Ok(DiagnosticSeverity::WARNING),
        "info" | "information" => Ok(DiagnosticSeverity::INFORMATION),
        "hint" => Ok(DiagnosticSeverity::HINT),
        other => Err(format!("unknown diagnostic severity {other}")),
    }
}

fn severity_to_string(severity: Option<DiagnosticSeverity>) -> &'static str {
    match severity.unwrap_or(DiagnosticSeverity::INFORMATION) {
        DiagnosticSeverity::ERROR => "error",
        DiagnosticSeverity::WARNING => "warning",
        DiagnosticSeverity::INFORMATION => "info",
        DiagnosticSeverity::HINT => "hint",
        _ => "info",
    }
}

fn string_field(map: &HashMap<String, Value>, name: &str) -> Result<String, String> {
    let Some(value) = map.get(name) else {
        return Err(format!("diagnostic requires {name}"));
    };
    let Value::String(value) = value else {
        return Err(format!("diagnostic {name} must be a string"));
    };
    Ok(value.to_string())
}

fn optional_string_field(
    map: &HashMap<String, Value>,
    name: &str,
) -> Result<Option<String>, String> {
    let Some(value) = map.get(name) else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value.to_string())),
        _ => Err(format!("diagnostic {name} must be a string or null")),
    }
}

fn range_from_value(value: &Value) -> Result<ScriptRange, String> {
    let Value::Map(map) = value else {
        return Err("range must be a map".to_string());
    };
    let start = map
        .get("start")
        .ok_or_else(|| "range requires start".to_string())?;
    let end = map
        .get("end")
        .ok_or_else(|| "range requires end".to_string())?;
    Ok(ScriptRange {
        start: cursor_from_value(start, "range.start")?,
        end: cursor_from_value(end, "range.end")?,
    })
}

fn cursor_from_value(value: &Value, label: &str) -> Result<Cursor, String> {
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    let row = map
        .get("row")
        .ok_or_else(|| format!("{label} requires row"))?;
    let col = map
        .get("col")
        .ok_or_else(|| format!("{label} requires col"))?;
    Ok(Cursor::new(
        usize_from_value(row, &format!("{label}.row"))?,
        usize_from_value(col, &format!("{label}.col"))?,
    ))
}

fn save_buffer_for_plugin(buffer_id: BufferId) -> Result<(), String> {
    let save_result = globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id));
    match save_result {
        Ok(()) => {
            globals::with_lsp_runtime_mut(|runtime| runtime.did_save_buffer(buffer_id));
            globals::enqueue_editor_event(EditorEvent::BufferSaved { buffer_id });
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(unknown_buffer_error(buffer_id))
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Err(format!(
            "buffer_id {} requires path before save",
            buffer_id.get()
        )),
        Err(error) => Err(format!(
            "failed to save buffer_id {}: {error}",
            buffer_id.get()
        )),
    }
}

pub(in crate::plugin) fn validate_callback(callback: &Value, label: &str) -> Result<(), String> {
    if matches!(callback, Value::ScriptFn(_) | Value::NativeFn(_)) {
        Ok(())
    } else {
        Err(format!("{label} must be a function"))
    }
}

fn hook_id_from_number(value: f64) -> Result<u64, String> {
    BearNumber::new(value, "event hook id")
        .non_negative_u64()
        .map_err(|_| format!("event hook id must be a non-negative integer, got {value}"))
}

pub(in crate::plugin) fn provider_id_from_number(value: f64) -> Result<u64, String> {
    BearNumber::new(value, "syntax provider id")
        .non_negative_u64()
        .map_err(|_| format!("syntax provider id must be a non-negative integer, got {value}"))
}

fn syntax_snapshot_to_value(
    buffer_id: BufferId,
    generation: u64,
    filetype: &str,
    path: Option<String>,
    text: &str,
    visible_range: Option<(usize, usize)>,
) -> Value {
    let lines = text
        .split('\n')
        .map(|line| Value::String(line.to_string().into_boxed_str().into()))
        .collect::<Vec<_>>();
    Value::Map(
        HashMap::from([
            (
                "buffer_id".to_string(),
                Value::Number(buffer_id.get() as f64),
            ),
            ("generation".to_string(), Value::Number(generation as f64)),
            ("filetype".to_string(), Value::String(filetype.into())),
            (
                "path".to_string(),
                path.map(|path| Value::String(path.into()))
                    .unwrap_or(Value::Null),
            ),
            ("text".to_string(), Value::String(text.into())),
            ("lines".to_string(), Value::List(lines.into())),
            (
                "visible_range".to_string(),
                visible_range
                    .map(|(start_row, end_row)| row_range_to_value(start_row, end_row))
                    .unwrap_or(Value::Null),
            ),
            ("changed_range".to_string(), Value::Null),
        ])
        .into(),
    )
}

fn syntax_line_spans_from_value(value: &Value, text: &str) -> Result<Vec<Vec<SyntaxSpan>>, String> {
    let lines = text.split('\n').collect::<Vec<_>>();
    let mut line_spans = vec![Vec::new(); lines.len()];
    let Value::List(spans) = value else {
        return Err("syntax provider must return a list of spans".to_string());
    };
    for (index, value) in spans.iter().enumerate() {
        let spans = syntax_spans_from_value(value, &lines)
            .map_err(|error| format!("syntax span {index}: {error}"))?;
        for (row, span) in spans {
            line_spans[row].push(span);
        }
    }
    for spans in &mut line_spans {
        spans.sort_by_key(|span| (span.start_byte, span.end_byte));
    }
    Ok(line_spans)
}

fn syntax_spans_from_value(
    value: &Value,
    lines: &[&str],
) -> Result<Vec<(usize, SyntaxSpan)>, String> {
    let Value::Map(map) = value else {
        return Err("span must be a map".to_string());
    };
    let range = range_from_value(
        map.get("range")
            .ok_or_else(|| "span requires range".to_string())?,
    )?;
    if range.start > range.end {
        return Err("span range start must be before or equal to range end".to_string());
    }
    let tag = match map.get("tag") {
        Some(Value::String(tag)) => urvim_theme::Tag::parse(tag)
            .map_err(|error| format!("invalid syntax tag {tag:?}: {error}"))?,
        _ => return Err("span requires string tag".to_string()),
    };
    split_syntax_span(range, tag, lines)
}

fn split_syntax_span(
    range: ScriptRange,
    tag: urvim_theme::Tag,
    lines: &[&str],
) -> Result<Vec<(usize, SyntaxSpan)>, String> {
    validate_span_endpoint(lines, range.start.line, range.start.col, "range.start")?;
    validate_span_endpoint(lines, range.end.line, range.end.col, "range.end")?;
    let mut spans = Vec::new();
    for row in range.start.line..=range.end.line {
        let line = lines[row];
        let start = if row == range.start.line {
            range.start.col
        } else {
            0
        };
        let end = if row == range.end.line {
            range.end.col
        } else {
            line.len()
        };
        if start < end {
            spans.push((row, SyntaxSpan::new(start, end, tag.clone())));
        }
    }
    Ok(spans)
}

fn validate_span_endpoint(
    lines: &[&str],
    row: usize,
    col: usize,
    label: &str,
) -> Result<(), String> {
    let line = lines
        .get(row)
        .ok_or_else(|| format!("{label} row {row} is out of range"))?;
    if col > line.len() {
        return Err(format!("{label} col {col} is out of range for row {row}"));
    }
    if !line.is_char_boundary(col) {
        return Err(format!(
            "{label} col {col} must be a UTF-8 character boundary"
        ));
    }
    Ok(())
}

/// Returns loaded buffer ids for command/status helpers.
pub(super) fn loaded_buffer_ids() -> BTreeSet<urvim_core::buffer::BufferId> {
    globals::with_buffer_pool(|pool| pool.buffer_ids().into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use urvim_core::WindowGroup;
    use urvim_core::buffer::Buffer;
    use urvim_core::editor::ModeKind;
    use urvim_core::ui::{Command, Intent};
    use urvim_terminal::{Key, KeyCode};

    fn shared_test_layout() -> SharedLayout {
        Rc::new(RefCell::new(Layout::new(WindowGroup::from_buffers(vec![
            Buffer::new(),
        ]))))
    }

    fn test_timers() -> Rc<PluginTimerRegistry> {
        Rc::new(PluginTimerRegistry::default())
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-{name}-{}-{stamp}", std::process::id()))
    }

    fn runtime_with_script(script: &str) -> BearscriptPluginRuntime {
        let root = unique_temp_dir("plugin-runtime-script-test");
        std::fs::create_dir_all(&root).expect("plugin dir should be created");
        std::fs::write(
            root.join("urvim-plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should be written");
        std::fs::write(root.join("plugin.bear"), script).expect("plugin script should be written");
        let plugin_id = "demo".to_string();
        let plugin_config = urvim_plugin::PluginConfigEntry {
            enabled: true,
            path: root.clone(),
        };
        let plugins = std::collections::BTreeMap::from([(plugin_id, plugin_config)]);
        let registry = urvim_plugin::PluginRegistry::load_from_config(&plugins)
            .expect("test plugin registry should load");
        let plugin = registry.get("demo").expect("demo plugin should load");
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime
            .load_plugin("demo", &plugin)
            .expect("test plugin should load");
        std::fs::remove_dir_all(root).ok();
        runtime
    }

    #[test]
    fn health_summary_counts_loaded_failed_and_callbacks() {
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.record_callback("loaded", "init", Duration::from_millis(1));
        runtime
            .health
            .entry("loaded".to_string())
            .or_default()
            .loaded = true;
        runtime.record_load_failure("failed", "boom".to_string());

        let summary = runtime.health_summary();

        assert_eq!(summary.loaded_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.callback_count, 1);
        assert_eq!(summary.max_callback, Duration::from_millis(1));
    }

    #[test]
    fn slow_callback_updates_health() {
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());

        runtime.record_callback("demo", "command slow", Duration::from_millis(50));

        let summary = runtime.health_summary();
        assert_eq!(summary.slow_callback_count, 1);
        assert_eq!(summary.callback_count, 1);
        assert_eq!(summary.max_callback, Duration::from_millis(50));
    }

    #[test]
    fn status_summary_reports_health_counts() {
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.record_callback("demo", "init", Duration::from_millis(2));
        runtime.health.entry("demo".to_string()).or_default().loaded = true;

        let status = runtime.status_summary();

        assert!(status.contains("1 loaded, 0 failed"));
        assert!(status.contains("1 callbacks"));
        assert!(status.contains("slowest 2ms"));
    }

    #[test]
    fn urvim_module_exposes_namespaced_api_surface() {
        let module = urvim_module(
            "demo".to_string(),
            Rc::new(RefCell::new(
                urvim_plugin::PluginContributionRegistry::default(),
            )),
            Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
            shared_test_layout(),
            Rc::new(PluginFsRegistry::default()),
            Rc::new(PluginJobRegistry::default()),
            Rc::new(PluginTimerRegistry::default()),
        );
        let Value::Module(module) = module else {
            panic!("urvim should be a module");
        };

        for name in [
            "events",
            "command",
            "register_event_hook",
            "unregister_event_hook",
            "buffers",
            "windows",
            "selection",
            "registers",
            "commands",
            "keymaps",
            "diagnostics",
            "ui",
            "strings",
            "path",
            "fs",
            "env",
            "filetypes",
            "json",
            "lists",
            "project",
            "jobs",
            "timers",
            "syntax",
            "inspect",
        ] {
            assert!(module.contains_key(name), "missing urvim.{name}");
        }
        for removed in [
            "notify",
            "register_command",
            "unregister_command",
            "str",
            "active_buffer",
            "list_buffers",
        ] {
            assert!(
                !module.contains_key(removed),
                "legacy urvim.{removed} should not be exposed"
            );
        }
    }

    #[test]
    fn urvim_events_module_keeps_existing_constants() {
        let Value::Module(events) = event_constants() else {
            panic!("events should be a module");
        };

        for event in [
            urvim_plugin::PluginEventKind::EditorStarted,
            urvim_plugin::PluginEventKind::BufferOpened,
            urvim_plugin::PluginEventKind::BufferLoaded,
            urvim_plugin::PluginEventKind::BufferSaved,
            urvim_plugin::PluginEventKind::BufferClosed,
            urvim_plugin::PluginEventKind::BufferUnloaded,
            urvim_plugin::PluginEventKind::BufferFiletypeChanged,
            urvim_plugin::PluginEventKind::CommandExecuted,
            urvim_plugin::PluginEventKind::DiagnosticsChanged,
        ] {
            assert_eq!(
                events.get(event.as_str()),
                Some(&Value::String(event.as_str().into()))
            );
        }
    }

    #[test]
    fn buffers_module_reads_buffer_state() {
        let _guard = buffer_pool_lock();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo",
        )]));
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.set_syntax_name("rust"));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::new(PluginTimerRegistry::default()),
            ),
        );

        let value = engine
            .eval(
                r#"
                [
                    urvim.buffers.active(),
                    urvim.buffers.exists(urvim.buffers.active()),
                    urvim.buffers.line_count(urvim.buffers.active()),
                    urvim.buffers.line(urvim.buffers.active(), 1),
                    urvim.buffers.text(urvim.buffers.active()),
                    urvim.buffers.filetype(urvim.buffers.active()),
                    urvim.buffers.is_modified(urvim.buffers.active())
                ]
                "#,
            )
            .expect("buffers API should read state");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::Number(buffer_id.get() as f64),
                    Value::Bool(true),
                    Value::Number(2.0),
                    Value::String("two".into()),
                    Value::String("one\ntwo".into()),
                    Value::String("rust".into()),
                    Value::Bool(false),
                ]
                .into()
            )
        );
    }

    #[test]
    fn buffers_set_filetype_accepts_plugin_filetypes() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::new(PluginTimerRegistry::default()),
            ),
        );

        engine
            .eval("urvim.buffers.set_filetype(urvim.buffers.active(), \"simplelang\")")
            .expect("custom plugin filetype should be accepted");

        assert_eq!(
            globals::with_buffer(buffer_id, |buffer| buffer.syntax_name().to_string()),
            Some("simplelang".to_string())
        );
    }

    #[test]
    fn syntax_module_registers_and_unregisters_provider() {
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::new(PluginTimerRegistry::default()),
            ),
        );

        let provider_id = engine
            .eval(
                r#"
                fn highlight(snapshot) { return [] }
                urvim.syntax.register("simplelang", highlight)
                "#,
            )
            .expect("syntax provider should register");
        assert_eq!(provider_id, Value::Number(0.0));
        assert!(
            contributions
                .borrow()
                .syntax_provider_for_filetype("simplelang")
                .is_some()
        );

        engine
            .eval("urvim.syntax.unregister(0)")
            .expect("syntax provider should unregister");
        assert!(
            contributions
                .borrow()
                .syntax_provider_for_filetype("simplelang")
                .is_none()
        );
        assert!(callbacks.borrow().syntax_providers.is_empty());
    }

    #[test]
    fn refresh_plugin_syntax_applies_provider_spans() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("fn demo");
        buffer.set_syntax_name("simplelang");
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            fn init() {
                urvim.syntax.register("simplelang", highlight)
            }

            fn highlight(snapshot) {
                return [{
                    "range": {
                        "start": { "row": 0, "col": 0 },
                        "end": { "row": 0, "col": 2 }
                    },
                    "tag": "syntax.keyword"
                }]
            }
            "#,
        );

        assert!(runtime.refresh_plugin_syntax());
        let spans = globals::with_buffer(buffer_id, |buffer| {
            buffer.cached_syntax_spans_for_line(0).unwrap_or_default()
        })
        .expect("buffer should exist");

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].end_byte, 2);
        assert_eq!(
            spans[0].style,
            urvim_theme::Tag::parse("syntax.keyword").unwrap()
        );
    }

    #[test]
    fn refresh_plugin_syntax_ignores_stale_provider_result() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("fn demo");
        buffer.set_syntax_name("simplelang");
        let generation = buffer.syntax_generation();
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            fn init() {
                urvim.syntax.register("simplelang", highlight)
            }

            fn highlight(snapshot) {
                urvim.buffers.set_line(snapshot["buffer_id"], 0, "let demo")
                return [{
                    "range": {
                        "start": { "row": 0, "col": 0 },
                        "end": { "row": 0, "col": 2 }
                    },
                    "tag": "syntax.keyword"
                }]
            }
            "#,
        );

        assert!(!runtime.refresh_plugin_syntax());
        let (current_generation, spans) = globals::with_buffer(buffer_id, |buffer| {
            (
                buffer.syntax_generation(),
                buffer.cached_syntax_spans_for_line(0),
            )
        })
        .expect("buffer should exist");

        assert_ne!(current_generation, generation);
        assert!(spans.is_none());
    }

    #[test]
    fn syntax_provider_snapshot_includes_lines() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("one\ntwo");
        buffer.set_syntax_name("simplelang");
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            fn init() {
                urvim.syntax.register("simplelang", highlight)
            }

            fn highlight(snapshot) {
                let line = snapshot["lines"][1]
                let word = byte_slice(line, 0, 3)
                if word != "two" {
                    return []
                }
                return [{
                    "range": {
                        "start": { "row": 1, "col": 0 },
                        "end": { "row": 1, "col": urvim.strings.byte_len(line) }
                    },
                    "tag": "syntax.string"
                }]
            }
            "#,
        );

        assert!(runtime.refresh_plugin_syntax());
        let spans = globals::with_buffer(buffer_id, |buffer| {
            buffer.cached_syntax_spans_for_line(1).unwrap_or_default()
        })
        .expect("buffer should exist");

        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].end_byte, 3);
    }

    #[test]
    fn syntax_provider_can_accumulate_spans_by_returning_lists() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("one\ntwo");
        buffer.set_syntax_name("simplelang");
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            fn init() {
                urvim.syntax.register("simplelang", highlight)
            }

            fn highlight(snapshot) {
                let spans = []
                let row = 0
                while row < len(snapshot["lines"]) {
                    spans = add_line_span(spans, row, snapshot["lines"][row])
                    row = row + 1
                }
                return spans
            }

            fn add_line_span(spans, row, line) {
                return urvim.lists.push(spans, {
                    "range": {
                        "start": { "row": row, "col": 0 },
                        "end": { "row": row, "col": urvim.strings.byte_len(line) }
                    },
                    "tag": "syntax.keyword"
                })
            }
            "#,
        );

        assert!(runtime.refresh_plugin_syntax());
        let (line0, line1) = globals::with_buffer(buffer_id, |buffer| {
            (
                buffer.cached_syntax_spans_for_line(0).unwrap_or_default(),
                buffer.cached_syntax_spans_for_line(1).unwrap_or_default(),
            )
        })
        .expect("buffer should exist");

        assert_eq!(line0.len(), 1);
        assert_eq!(line1.len(), 1);
        assert_eq!((line0[0].start_byte, line0[0].end_byte), (0, 3));
        assert_eq!((line1[0].start_byte, line1[0].end_byte), (0, 3));
    }

    #[test]
    fn syntax_refresh_api_forces_provider_execution() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("one");
        buffer.set_syntax_name("simplelang");
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            let calls = 0
            fn init() {
                urvim.syntax.register("simplelang", highlight)
                urvim.syntax.refresh(urvim.buffers.active())
            }

            fn highlight(snapshot) {
                calls = calls + 1
                return [{
                    "range": {
                        "start": { "row": 0, "col": 0 },
                        "end": { "row": 0, "col": 3 }
                    },
                    "tag": "syntax.keyword"
                }]
            }
            "#,
        );

        runtime.refresh_plugin_syntax();
        let calls = runtime
            .plugins
            .get_mut("demo")
            .expect("plugin should exist")
            .engine
            .eval("calls")
            .expect("calls should evaluate");

        let Value::Number(calls) = calls else {
            panic!("calls should be numeric");
        };
        assert!(calls >= 1.0);
    }

    #[test]
    fn syntax_provider_multiline_spans_are_split_by_line() {
        let _guard = buffer_pool_lock();
        let mut buffer = Buffer::from_str("alpha\nbeta");
        buffer.set_syntax_name("simplelang");
        let layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut runtime = runtime_with_script(
            r#"
            fn init() {
                urvim.syntax.register("simplelang", highlight)
            }

            fn highlight(snapshot) {
                return [{
                    "range": {
                        "start": { "row": 0, "col": 2 },
                        "end": { "row": 1, "col": 2 }
                    },
                    "tag": "syntax.comment"
                }]
            }
            "#,
        );

        assert!(runtime.refresh_plugin_syntax());
        let (line0, line1) = globals::with_buffer(buffer_id, |buffer| {
            (
                buffer.cached_syntax_spans_for_line(0).unwrap_or_default(),
                buffer.cached_syntax_spans_for_line(1).unwrap_or_default(),
            )
        })
        .expect("buffer should exist");

        assert_eq!((line0[0].start_byte, line0[0].end_byte), (2, 5));
        assert_eq!((line1[0].start_byte, line1[0].end_byte), (0, 2));
    }

    #[test]
    fn filetypes_module_registers_extension_detection() {
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval(
                r#"
                urvim.filetypes.register("simplelang")
                urvim.filetypes.detect_extension(".simple", "simplelang")
                "#,
            )
            .expect("filetype APIs should register detection");

        assert_eq!(
            contributions.borrow().filetype_for_extension("simple"),
            Some("simplelang")
        );
    }

    #[test]
    fn buffers_module_mutates_lines_and_ranges() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval(
                r#"
                let id = urvim.buffers.active()
                urvim.buffers.set_line(id, 0, "ONE")
                urvim.buffers.insert_line(id, 1, "middle")
                urvim.buffers.delete_line(id, 2)
                urvim.buffers.replace_range(id, {
                    "start": { "row": 1, "col": 0 },
                    "end": { "row": 1, "col": 6 }
                }, "TWO")
                "#,
            )
            .expect("buffers API should mutate state");

        let text =
            globals::with_buffer(buffer_id, |buffer| buffer.as_str()).expect("buffer should exist");
        assert_eq!(text, "ONE\nTWO");
    }

    #[test]
    fn buffers_module_errors_for_missing_buffer_and_out_of_range_row() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let missing = engine
            .eval("urvim.buffers.line(999999, 0)")
            .expect_err("missing buffer should error")
            .to_string();
        assert!(missing.contains("unknown buffer_id 999999"));

        let out_of_range = engine
            .eval("urvim.buffers.line(urvim.buffers.active(), 3)")
            .expect_err("out of range row should error")
            .to_string();
        assert!(out_of_range.contains("row 3 is out of range"));
    }

    #[test]
    fn buffers_module_save_uses_editor_save_notifications() {
        let _guard = buffer_pool_lock();
        globals::clear_editor_events_for_tests();
        let unique = format!(
            "urvim-buffers-api-save-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let absolute_path = urvim_core::AbsolutePath::from_path(path.as_path())
            .expect("temp path should resolve absolutely");
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str_with_path(
            "saved text",
            absolute_path,
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval("urvim.buffers.save(urvim.buffers.active())")
            .expect("save should succeed");

        let saved_text = std::fs::read_to_string(&path).expect("saved file should be readable");
        assert_eq!(saved_text, "saved text");
        let events = drain_editor_events();
        assert!(events.iter().any(|event| matches!(
            event,
            EditorEvent::BufferSaved { buffer_id: id } if *id == buffer_id
        )));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn windows_module_reads_active_window_state() {
        let _guard = buffer_pool_lock();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo\nthree",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        layout
            .active_buffer_view_mut()
            .set_cursor(Cursor::new(1, 2));
        globals::set_active_buffer_id(buffer_id);
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let win = urvim.windows.active()
                [
                    win,
                    urvim.windows.list(),
                    urvim.windows.buffer(win),
                    urvim.windows.cursor(win)
                ]
                "#,
            )
            .expect("windows API should read active window state");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::Number(0.0),
                    Value::List(vec![Value::Number(0.0)].into()),
                    Value::Number(buffer_id.get() as f64),
                    Value::Map(
                        HashMap::from([
                            ("row".to_string(), Value::Number(1.0)),
                            ("col".to_string(), Value::Number(2.0)),
                        ])
                        .into()
                    ),
                ]
                .into()
            )
        );
    }

    #[test]
    fn windows_module_sets_cursor_and_opens_loaded_buffer() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));
        let visible_id = layout.active_buffer_view().buffer_id();
        let hidden_id = globals::with_buffer_pool(|pool| pool.create_buffer());
        globals::set_active_buffer_id(visible_id);
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval(&format!(
                r#"
                let win = urvim.windows.active()
                urvim.windows.set_cursor(win, 0, 3)
                urvim.windows.open_buffer({})
                "#,
                hidden_id.get()
            ))
            .expect("windows API should mutate active layout");

        let active_buffer = layout.borrow().active_buffer_view().buffer_id();
        assert_eq!(active_buffer, hidden_id);
        assert_eq!(globals::with_active_buffer_id(|id| id), Some(hidden_id));
    }

    #[test]
    fn windows_module_exposes_distinct_split_window_ids() {
        let _guard = buffer_pool_lock();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo\nthree",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));
        layout
            .buffer_view_for_window_mut(urvim_core::layout::PaneId(0))
            .expect("original window should exist")
            .set_cursor(Cursor::new(0, 1));
        layout
            .buffer_view_for_window_mut(urvim_core::layout::PaneId(1))
            .expect("split window should exist")
            .set_cursor(Cursor::new(1, 2));
        globals::set_active_buffer_id(buffer_id);
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let wins = urvim.windows.list()
                urvim.windows.set_cursor(wins[0], 2, 0)
                [urvim.windows.active(), wins, urvim.windows.cursor(wins[0]), urvim.windows.cursor(wins[1])]
                "#,
            )
            .expect("windows API should address split windows independently");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::Number(1.0),
                    Value::List(vec![Value::Number(0.0), Value::Number(1.0)].into()),
                    Value::Map(
                        HashMap::from([
                            ("row".to_string(), Value::Number(2.0)),
                            ("col".to_string(), Value::Number(0.0)),
                        ])
                        .into()
                    ),
                    Value::Map(
                        HashMap::from([
                            ("row".to_string(), Value::Number(1.0)),
                            ("col".to_string(), Value::Number(2.0)),
                        ])
                        .into()
                    ),
                ]
                .into()
            )
        );
    }

    #[test]
    fn windows_module_visible_range_stays_within_buffer_lines() {
        let _guard = buffer_pool_lock();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo\nthree",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        layout
            .active_buffer_view_mut()
            .set_scroll_offset(urvim_core::window::Position::new(1, 0));
        globals::set_active_buffer_id(buffer_id);
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval("urvim.windows.visible_range(urvim.windows.active())")
            .expect("visible_range should return viewport rows");

        assert_eq!(
            value,
            Value::Map(
                HashMap::from([
                    ("start_row".to_string(), Value::Number(1.0)),
                    ("end_row".to_string(), Value::Number(1.0)),
                ])
                .into()
            )
        );
    }

    #[test]
    fn selection_module_returns_null_without_active_selection() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                layout,
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval("[urvim.selection.get(), urvim.selection.text()]")
            .expect("selection API should report no active selection");

        assert_eq!(value, Value::List(vec![Value::Null, Value::Null].into()));
    }

    #[test]
    fn selection_module_sets_gets_text_and_clears_selection() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                urvim.selection.set({
                    "start": { "row": 0, "col": 1 },
                    "end": { "row": 0, "col": 4 }
                })
                let before = [urvim.selection.get(), urvim.selection.text()]
                urvim.selection.clear()
                [before, urvim.selection.get()]
                "#,
            )
            .expect("selection API should set, read, and clear selection");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::List(
                        vec![
                            Value::Map(
                                HashMap::from([
                                    (
                                        "start".to_string(),
                                        Value::Map(
                                            HashMap::from([
                                                ("row".to_string(), Value::Number(0.0)),
                                                ("col".to_string(), Value::Number(1.0)),
                                            ])
                                            .into()
                                        ),
                                    ),
                                    (
                                        "end".to_string(),
                                        Value::Map(
                                            HashMap::from([
                                                ("row".to_string(), Value::Number(0.0)),
                                                ("col".to_string(), Value::Number(4.0)),
                                            ])
                                            .into()
                                        ),
                                    ),
                                ])
                                .into()
                            ),
                            Value::String("ell".into()),
                        ]
                        .into()
                    ),
                    Value::Null,
                ]
                .into()
            )
        );
    }

    #[test]
    fn selection_module_replaces_selection_and_errors_without_selection() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval(
                r#"
                urvim.selection.set({
                    "start": { "row": 0, "col": 1 },
                    "end": { "row": 0, "col": 4 }
                })
                urvim.selection.replace("ipp")
                "#,
            )
            .expect("selection.replace should edit the buffer");

        let text = globals::with_buffer(buffer_id, |buffer| buffer.as_str())
            .expect("buffer should still exist");
        assert_eq!(text, "hippo");
        assert!(
            layout
                .borrow()
                .active_buffer_view()
                .visual_selection()
                .is_none()
        );

        let missing = engine
            .eval("urvim.selection.replace(\"x\")")
            .expect_err("selection.replace should require an active selection")
            .to_string();
        assert!(missing.contains("no active selection"));
    }

    #[test]
    fn selection_module_errors_for_invalid_range() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                layout,
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let out_of_range = engine
            .eval(
                r#"
                urvim.selection.set({
                    "start": { "row": 0, "col": 1 },
                    "end": { "row": 20, "col": 0 }
                })
                "#,
            )
            .expect_err("selection.set should validate range endpoints")
            .to_string();
        assert!(out_of_range.contains("range.end row 20 col 0 is out of range"));
    }

    #[test]
    fn registers_module_sets_gets_appends_and_lists_registers() {
        let _guard = buffer_pool_lock();
        globals::with_register_store_mut(|store| {
            store.clear(urvim_core::register::RegisterName::new('a'));
            store.clear(urvim_core::register::RegisterName::new('b'));
            store.clear(urvim_core::register::RegisterName::UNNAMED);
        });
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let missing = urvim.registers.get("a")
                urvim.registers.set("a", "hello")
                urvim.registers.append("a", " world")
                urvim.registers.set("\"", "unnamed")
                [missing, urvim.registers.get("a"), urvim.registers.get("\""), urvim.registers.names()]
                "#,
            )
            .expect("registers API should mutate and list registers");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::String("".into()),
                    Value::String("hello world".into()),
                    Value::String("unnamed".into()),
                    Value::List(vec![Value::String("\"".into()), Value::String("a".into())].into()),
                ]
                .into()
            )
        );
    }

    #[test]
    fn registers_module_errors_for_invalid_register_names() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let uppercase = engine
            .eval("urvim.registers.get(\"A\")")
            .expect_err("uppercase register should be rejected")
            .to_string();
        assert!(uppercase.contains("invalid register name A"));

        let multi_char = engine
            .eval("urvim.registers.set(\"ab\", \"value\")")
            .expect_err("multi-character register should be rejected")
            .to_string();
        assert!(multi_char.contains("register name must be one character"));
    }

    #[test]
    fn commands_module_registers_lists_and_unregisters_commands() {
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                fn hello(args) {
                    urvim.ui.show_message("hello", { "level": "info" })
                }
                fn quiet(args) {}
                urvim.commands.register("hello", hello, "Say hello")
                urvim.commands.register("quiet", quiet)
                let before = urvim.commands.list()
                urvim.commands.unregister("hello")
                urvim.commands.unregister("quiet")
                [before, urvim.commands.list()]
                "#,
            )
            .expect("commands namespace should register, list, and unregister commands");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::List(
                        vec![
                            Value::Map(
                                HashMap::from([
                                    ("name".to_string(), Value::String("hello".into())),
                                    ("description".to_string(), Value::String("Say hello".into())),
                                ])
                                .into()
                            ),
                            Value::Map(
                                HashMap::from([
                                    ("name".to_string(), Value::String("quiet".into())),
                                    ("description".to_string(), Value::Null),
                                ])
                                .into()
                            ),
                        ]
                        .into()
                    ),
                    Value::List(vec![].into()),
                ]
                .into()
            )
        );
        assert!(contributions.borrow().command("demo", "hello").is_none());
        assert!(contributions.borrow().command("demo", "quiet").is_none());
        assert!(!callbacks.borrow().commands.contains_key("hello"));
        assert!(!callbacks.borrow().commands.contains_key("quiet"));
    }

    #[test]
    fn commands_module_rejects_invalid_callbacks_and_names() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let invalid_callback = engine
            .eval("urvim.commands.register(\"hello\", 1, null)")
            .expect_err("commands.register should require function callbacks")
            .to_string();
        assert!(invalid_callback.contains("command callback must be a function"));

        let invalid_name = engine
            .eval(
                r#"
                fn hello(args) {}
                urvim.commands.register("bad name", hello, null)
                "#,
            )
            .expect_err("commands.register should validate command names")
            .to_string();
        assert!(invalid_name.contains("plugin command name"));
    }

    #[test]
    fn string_modules_provide_utility_helpers() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                [
                    urvim.strings.trim("  main.rs  "),
                    urvim.strings.trim_start("  main.rs  "),
                    urvim.strings.trim_end("  main.rs  "),
                    urvim.strings.starts_with("main.rs", "main"),
                    urvim.strings.ends_with("main.rs", ".rs"),
                    urvim.strings.contains("main.rs", "in."),
                    urvim.strings.split("a,b,c", ","),
                    urvim.strings.join(["a", "b", "c"], ":"),
                    urvim.strings.replace("main.rs", ".rs", ".toml"),
                    urvim.strings.to_lower("MAIN.RS"),
                    urvim.strings.to_upper("main.rs"),
                    urvim.strings.len("hé"),
                    urvim.strings.byte_len("hé"),
                    urvim.strings.char_at("simple", 2),
                    urvim.strings.find("simple", "pl", 0),
                    urvim.strings.find("simple", "zz", 0)
                ]
                "#,
            )
            .expect("string helpers should evaluate");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::String("main.rs".into()),
                    Value::String("main.rs  ".into()),
                    Value::String("  main.rs".into()),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::List(
                        vec![
                            Value::String("a".into()),
                            Value::String("b".into()),
                            Value::String("c".into()),
                        ]
                        .into()
                    ),
                    Value::String("a:b:c".into()),
                    Value::String("main.toml".into()),
                    Value::String("main.rs".into()),
                    Value::String("MAIN.RS".into()),
                    Value::Number(2.0),
                    Value::Number(3.0),
                    Value::String("m".into()),
                    Value::Number(3.0),
                    Value::Number(-1.0),
                ]
                .into()
            )
        );
    }

    #[test]
    fn path_module_provides_path_string_helpers() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let path = urvim.path.join(["src", "main.rs"])
                [
                    path,
                    urvim.path.dirname(path),
                    urvim.path.basename(path),
                    urvim.path.extension(path),
                    urvim.path.extension("Makefile"),
                    urvim.path.stem(path)
                ]
                "#,
            )
            .expect("path helpers should evaluate");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::String("src/main.rs".into()),
                    Value::String("src".into()),
                    Value::String("main.rs".into()),
                    Value::String("rs".into()),
                    Value::Null,
                    Value::String("main".into()),
                ]
                .into()
            )
        );
    }

    #[test]
    fn env_module_reads_environment_variables() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(r#"urvim.env.get("URVIM_TEST_ENV_VALUE_THAT_SHOULD_NOT_EXIST")"#)
            .expect("env helper should evaluate");

        assert_eq!(value, Value::Null);
    }

    #[test]
    fn json_module_parses_and_stringifies_values() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        engine.set_global(
            "json_text",
            Value::String(r#"{"name":"demo","count":2,"ok":true,"items":["a",null]}"#.into()),
        );

        let value = engine
            .eval(
                r#"
                let parsed = urvim.json.parse(json_text)
                [
                    parsed["name"],
                    parsed["count"],
                    parsed["ok"],
                    parsed["items"][1],
                    urvim.strings.contains(urvim.json.stringify(parsed), "\"name\":"),
                    urvim.strings.contains(urvim.json.stringify_pretty(parsed), "\n")
                ]
                "#,
            )
            .expect("json helpers should evaluate");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::String("demo".into()),
                    Value::Number(2.0),
                    Value::Bool(true),
                    Value::Null,
                    Value::Bool(true),
                    Value::Bool(true),
                ]
                .into()
            )
        );
    }

    #[test]
    fn fs_module_reads_basic_metadata() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        let root = std::env::current_dir().expect("current dir should exist");
        let cargo_toml = root.join("Cargo.toml");
        engine.set_global(
            "root",
            Value::String(root.to_string_lossy().to_string().into_boxed_str().into()),
        );
        engine.set_global(
            "cargo_toml",
            Value::String(
                cargo_toml
                    .to_string_lossy()
                    .to_string()
                    .into_boxed_str()
                    .into(),
            ),
        );

        let value = engine
            .eval(
                r#"
                [
                    urvim.fs.exists(root),
                    urvim.fs.is_dir(root),
                    urvim.fs.is_file(root),
                    urvim.fs.exists(cargo_toml),
                    urvim.fs.is_file(cargo_toml),
                    urvim.fs.is_dir(cargo_toml)
                ]
                "#,
            )
            .expect("fs helpers should evaluate");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::Bool(false),
                ]
                .into()
            )
        );
    }

    #[test]
    fn fs_module_dispatches_read_file_callback() {
        let temp_dir =
            std::env::temp_dir().join(format!("urvim-fs-read-file-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("read.txt");
        std::fs::write(&path, "hello from fs").expect("test file should be written");
        let mut runtime = runtime_with_fs_script(&format!(
            r#"
            let fs_result = null
            fn on_read(result) {{
                fs_result = result["text"]
            }}
            urvim.fs.read_file("{}", on_read)
            "#,
            path.to_string_lossy()
        ));

        dispatch_fs_until_global(
            &mut runtime,
            "fs_result",
            Value::String("hello from fs".into()),
        );
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn fs_module_dispatches_write_file_callback() {
        let temp_dir =
            std::env::temp_dir().join(format!("urvim-fs-write-file-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let path = temp_dir.join("write.txt");
        let mut runtime = runtime_with_fs_script(&format!(
            r#"
            let fs_result = null
            fn on_write(result) {{
                fs_result = result["path"]
            }}
            urvim.fs.write_file("{}", "written text", on_write)
            "#,
            path.to_string_lossy()
        ));

        dispatch_fs_until_global(
            &mut runtime,
            "fs_result",
            Value::String(path.to_string_lossy().to_string().into_boxed_str().into()),
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("written file should be readable"),
            "written text"
        );
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn fs_module_dispatches_read_dir_callback() {
        let temp_dir =
            std::env::temp_dir().join(format!("urvim-fs-read-dir-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        std::fs::write(temp_dir.join("alpha.txt"), "alpha").expect("file should be written");
        std::fs::create_dir_all(temp_dir.join("nested")).expect("nested dir should be created");
        let mut runtime = runtime_with_fs_script(&format!(
            r#"
            let fs_result = null
            fn on_read_dir(result) {{
                fs_result = result["entries"][0]["kind"]
            }}
            urvim.fs.read_dir("{}", on_read_dir)
            "#,
            temp_dir.to_string_lossy()
        ));

        dispatch_fs_until_global(&mut runtime, "fs_result", Value::String("file".into()));
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn fs_module_dispatches_error_payload() {
        let path = std::env::temp_dir().join(format!("urvim-fs-missing-{}", std::process::id()));
        let mut runtime = runtime_with_fs_script(&format!(
            r#"
            let fs_result = null
            fn on_error(result) {{
                fs_result = result["ok"]
            }}
            urvim.fs.read_file("{}", on_error)
            "#,
            path.to_string_lossy()
        ));

        dispatch_fs_until_global(&mut runtime, "fs_result", Value::Bool(false));
    }

    #[test]
    fn project_module_finds_markers_upward() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        let root = std::env::current_dir().expect("current dir should exist");
        let start = root.join("crates").join("urvim").join("src");
        let cargo_toml = root.join("Cargo.toml");
        engine.set_global(
            "start",
            Value::String(start.to_string_lossy().to_string().into_boxed_str().into()),
        );

        let value = engine
            .eval(
                r#"
                [
                    urvim.project.find_up("Cargo.toml", start),
                    urvim.project.root(["Cargo.toml", ".git"], start),
                    urvim.project.find_up("definitely-not-a-real-marker", start)
                ]
                "#,
            )
            .expect("project helpers should evaluate");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::String(
                        cargo_toml
                            .to_string_lossy()
                            .to_string()
                            .into_boxed_str()
                            .into()
                    ),
                    Value::String(root.to_string_lossy().to_string().into_boxed_str().into()),
                    Value::Null,
                ]
                .into()
            )
        );
    }

    #[test]
    fn inspect_function_formats_values_for_debugging() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(r#"urvim.inspect(["a", 2, true])"#)
            .expect("inspect should evaluate");

        assert_eq!(value, Value::String("[a, 2, true]".into()));
    }

    #[test]
    fn jobs_module_spawns_and_dispatches_callbacks() {
        globals::clear_notifications();
        let jobs = Rc::new(PluginJobRegistry::default());
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.jobs = Rc::clone(&jobs);
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::clone(&jobs),
                test_timers(),
            ),
        );
        engine
            .eval(
                r#"
                fn on_stdout(event) {
                    urvim.ui.show_message(event["text"], { "level": "info" })
                }
                fn on_exit(event) {
                    urvim.ui.show_message(event["status"], { "level": "info" })
                }
                urvim.jobs.spawn({
                    "cmd": "sh",
                    "args": ["-c", "printf hello"],
                    "on_stdout": on_stdout,
                    "on_exit": on_exit
                })
                "#,
            )
            .expect("job should spawn");
        runtime
            .plugins
            .insert("demo".to_string(), BearscriptPlugin { engine, callbacks });

        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            runtime.dispatch_job_events();
            if globals::active_notification(Instant::now()).is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for plugin job callbacks");
    }

    #[test]
    fn timers_module_defers_and_dispatches_callbacks() {
        globals::clear_notifications();
        let timers = Rc::new(PluginTimerRegistry::default());
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.timers = Rc::clone(&timers);
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::clone(&timers),
            ),
        );
        let timer_id = engine
            .eval(
                r#"
                urvim.timers.defer(fn() {
                    urvim.ui.show_message("deferred", { "level": "info" })
                })
                "#,
            )
            .expect("defer should return a timer id");
        assert_eq!(timer_id, Value::Number(1.0));
        runtime
            .plugins
            .insert("demo".to_string(), BearscriptPlugin { engine, callbacks });

        assert!(runtime.dispatch_timer_events());

        let notification = globals::active_notification(Instant::now())
            .expect("timer callback should show a notification");
        assert_eq!(notification.text, "deferred");
        globals::clear_notifications();
    }

    #[test]
    fn timers_module_runs_timeout_and_clear_prevents_dispatch() {
        globals::clear_notifications();
        let timers = Rc::new(PluginTimerRegistry::default());
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.timers = Rc::clone(&timers);
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::clone(&timers),
            ),
        );
        engine
            .eval(
                r#"
                urvim.timers.set_timeout(5, fn() {
                    urvim.ui.show_message("timeout", { "level": "info" })
                })
                let cleared = urvim.timers.set_timeout(5, fn() {
                    urvim.ui.show_message("cleared", { "level": "info" })
                })
                urvim.timers.clear(cleared)
                "#,
            )
            .expect("timeouts should register and clear");
        runtime
            .plugins
            .insert("demo".to_string(), BearscriptPlugin { engine, callbacks });

        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            runtime.dispatch_timer_events();
            if let Some(notification) = globals::active_notification(Instant::now())
                && notification.text == "timeout"
            {
                globals::clear_notifications();
                assert!(callbacks_absent_for_timer(&runtime, "demo", 1));
                assert!(callbacks_absent_for_timer(&runtime, "demo", 2));
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out waiting for plugin timeout callback");
    }

    #[test]
    fn timers_module_interval_repeats_until_cleared() {
        globals::clear_notifications();
        let timers = Rc::new(PluginTimerRegistry::default());
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.timers = Rc::clone(&timers);
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::clone(&callbacks),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                Rc::clone(&timers),
            ),
        );
        let timer_id = engine
            .eval(
                r#"
                urvim.timers.set_interval(5, fn() {
                    urvim.ui.show_message("tick", { "level": "info" })
                })
                "#,
            )
            .expect("interval should return a timer id");
        assert_eq!(timer_id, Value::Number(1.0));
        runtime
            .plugins
            .insert("demo".to_string(), BearscriptPlugin { engine, callbacks });

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut ticks = 0;
        while Instant::now() < deadline {
            runtime.dispatch_timer_events();
            if globals::active_notification(Instant::now()).is_some() {
                ticks += 1;
                globals::clear_notifications();
                if ticks == 2 {
                    let plugin_runtime = runtime
                        .plugins
                        .get_mut("demo")
                        .expect("plugin should be loaded");
                    plugin_runtime
                        .engine
                        .eval("urvim.timers.clear(1)")
                        .expect("clear should cancel interval");
                    assert!(callbacks_absent_for_timer(&runtime, "demo", 1));
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out waiting for repeated interval callback");
    }

    #[test]
    fn timers_module_rejects_invalid_arguments() {
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let callback = engine
            .eval("urvim.timers.defer(1)")
            .expect_err("defer should require a callback")
            .to_string();
        assert!(callback.contains("timer callback must be a function"));

        let delay = engine
            .eval("urvim.timers.set_timeout(-1, fn() {})")
            .expect_err("set_timeout should reject invalid delay")
            .to_string();
        assert!(delay.contains("timer delay must be a non-negative integer"));
    }

    #[test]
    fn command_execution_runs_safe_command_and_enqueues_event() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let layout = Rc::new(RefCell::new(layout));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval("urvim.command(\"pane wrap-toggle\")")
            .expect("urvim.command should execute safe commands");

        assert_eq!(value, Value::Bool(true));
        let events = drain_editor_events();
        assert!(events.iter().any(|event| matches!(
            event,
            EditorEvent::CommandExecuted { command } if command.contains("ToggleWrap")
        )));
    }

    #[test]
    fn commands_execute_uses_same_execution_path() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval("urvim.commands.execute(\"pane wrap-toggle\")")
            .expect("commands.execute should execute safe commands");

        assert_eq!(value, Value::Bool(true));
    }

    #[test]
    fn command_execution_accepts_buffer_id_from_bearscript_api() {
        let _guard = buffer_pool_lock();
        globals::clear_editor_events_for_tests();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let layout = Rc::new(RefCell::new(layout));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let id = urvim.buffers.active()
                urvim.command("buffer filetype rust buffer={id}")
                "#,
            )
            .expect("buffer id should be accepted by command execution");

        assert_eq!(value, Value::Bool(true));
        assert_eq!(
            globals::with_buffer(buffer_id, |buffer| buffer.syntax_name().to_string()),
            Some("rust".to_string())
        );
        let events = drain_editor_events();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    EditorEvent::BufferFiletypeChanged { buffer_id: id } if *id == buffer_id
                ))
                .count(),
            1
        );

        engine
            .eval(
                r#"
                let id = urvim.buffers.active()
                urvim.commands.execute("buffer filetype rust buffer={id}")
                "#,
            )
            .expect("unchanged filetype command should be handled");
        assert!(drain_editor_events().iter().all(|event| !matches!(
            event,
            EditorEvent::BufferFiletypeChanged { buffer_id: id } if *id == buffer_id
        )));
    }

    #[test]
    fn command_execution_reports_missing_buffer_targets_without_events() {
        let _guard = buffer_pool_lock();
        globals::clear_editor_events_for_tests();
        globals::clear_notifications();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        let missing = usize::MAX;

        for command in [
            format!("buffer close buffer={missing}"),
            format!("buffer unload force=true buffer={missing}"),
            format!("buffer filetype rust buffer={missing}"),
        ] {
            assert_eq!(
                engine
                    .eval(&format!("urvim.command(\"{command}\")"))
                    .expect("missing target command should be handled"),
                Value::Bool(true)
            );
            assert_eq!(
                globals::active_notification(std::time::Instant::now())
                    .expect("missing target should notify")
                    .text,
                format!("Unknown buffer: {missing}")
            );
            globals::clear_notifications();
        }

        assert!(drain_editor_events().iter().all(|event| !matches!(
            event,
            EditorEvent::BufferClosed { buffer_id }
                | EditorEvent::BufferUnloaded { buffer_id, .. }
                | EditorEvent::BufferFiletypeChanged { buffer_id }
                if buffer_id.get() == missing
        )));
    }

    #[test]
    fn command_execution_returns_false_for_unhandled_commands() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval("urvim.command(\"pane focus-left\")")
            .expect("valid unhandled commands should return false");

        assert_eq!(value, Value::Bool(false));
    }

    #[test]
    fn command_execution_rejects_unknown_plugin_and_quit_commands() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let unknown = engine
            .eval("urvim.command(\"nope\")")
            .expect_err("unknown command should error")
            .to_string();
        assert!(unknown.contains("unknown command") || unknown.contains("Unknown command"));

        let plugin = engine
            .eval("urvim.command(\"plugin status\")")
            .expect_err("plugin commands should be rejected")
            .to_string();
        assert!(plugin.contains("does not allow plugin commands"));

        let quit = engine
            .eval("urvim.command(\"quit\")")
            .expect_err("quit commands should be rejected")
            .to_string();
        assert!(quit.contains("does not allow quit commands"));
    }

    #[test]
    fn keymaps_module_sets_lists_and_deletes_normal_mode_mapping() {
        let _guard = buffer_pool_lock();
        globals::with_plugin_keymaps_mut(|keymaps| keymaps.normal.clear());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                urvim.keymaps.set("normal", "x", "pane wrap-toggle")
                let before = urvim.keymaps.list("normal")
                let all = urvim.keymaps.list()
                urvim.keymaps.delete("normal", "x")
                [before, all, urvim.keymaps.list("normal")]
                "#,
            )
            .expect("keymaps API should set, list, and delete mappings");

        assert_eq!(
            value,
            Value::List(
                vec![
                    Value::List(
                        vec![Value::Map(
                            HashMap::from([
                                ("mode".to_string(), Value::String("normal".into())),
                                ("lhs".to_string(), Value::String("x".into())),
                                ("rhs".to_string(), Value::String("pane wrap-toggle".into())),
                            ])
                            .into()
                        )]
                        .into()
                    ),
                    Value::List(
                        vec![Value::Map(
                            HashMap::from([
                                ("mode".to_string(), Value::String("normal".into())),
                                ("lhs".to_string(), Value::String("x".into())),
                                ("rhs".to_string(), Value::String("pane wrap-toggle".into())),
                            ])
                            .into()
                        )]
                        .into()
                    ),
                    Value::List(vec![].into()),
                ]
                .into()
            )
        );
    }

    #[test]
    fn keymaps_module_invokes_configured_command_string() {
        let _guard = buffer_pool_lock();
        globals::with_plugin_keymaps_mut(|keymaps| keymaps.normal.clear());
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        engine
            .eval("urvim.keymaps.set(\"normal\", \"x\", \"pane wrap-toggle\")")
            .expect("keymap should be installed");
        layout
            .active_window_group_mut()
            .active_window_mut()
            .switch_mode(ModeKind::Normal);

        let result = layout
            .active_window_group_mut()
            .active_window_mut()
            .handle_key(&urvim_terminal::Key::new(urvim_terminal::KeyCode::Char(
                'x',
            )));

        assert_eq!(
            result,
            urvim_core::editor::HandleKeyResult::Complete(Intent::Command(Command::ToggleWrap))
        );
    }

    #[test]
    fn keymaps_module_errors_for_invalid_mode_lhs_rhs_and_opts() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let mode = engine
            .eval("urvim.keymaps.set(\"bad\", \"x\", \"pane wrap-toggle\")")
            .expect_err("invalid mode should error")
            .to_string();
        assert!(mode.contains("unknown keymap mode bad"));

        let lhs = engine
            .eval("urvim.keymaps.set(\"normal\", \"\", \"pane wrap-toggle\")")
            .expect_err("invalid lhs should error")
            .to_string();
        assert!(lhs.contains("key string must not be empty"));

        let rhs = engine
            .eval("urvim.keymaps.set(\"normal\", \"x\", \"plugin status\")")
            .expect_err("plugin command rhs should error")
            .to_string();
        assert!(rhs.contains("does not allow plugin commands"));

        engine
            .eval("urvim.keymaps.set(\"normal\", \"x\", \"pane wrap-toggle\", {})")
            .expect("empty opts should be accepted");

        let bad_opts = engine
            .eval("urvim.keymaps.set(\"normal\", \"x\", \"pane wrap-toggle\", 1)")
            .expect_err("non-map opts should error")
            .to_string();
        assert!(bad_opts.contains("keymap opts must be a map or null"));

        let unsupported_opts = engine
            .eval("urvim.keymaps.set(\"normal\", \"x\", \"pane wrap-toggle\", { \"desc\": \"Toggle wrap\" })")
            .expect_err("unsupported opts should error")
            .to_string();
        assert!(unsupported_opts.contains("unknown keymap option desc"));
    }

    #[test]
    fn diagnostics_module_sets_gets_filters_counts_and_clears() {
        let _guard = buffer_pool_lock();
        globals::with_diagnostics_store(|store| store.clear_all());
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(&format!(
                r#"
                let id = {}
                urvim.diagnostics.set("ns-a", id, [{{
                    "range": {{ "start": {{ "row": 0, "col": 0 }}, "end": {{ "row": 0, "col": 5 }} }},
                    "severity": "error",
                    "message": "boom",
                    "source": "lint-a"
                }}])
                urvim.diagnostics.set("ns-b", id, [{{
                    "range": {{ "start": {{ "row": 0, "col": 1 }}, "end": {{ "row": 0, "col": 2 }} }},
                    "severity": "warning",
                    "message": "careful"
                }}])
                let all = urvim.diagnostics.get(id)
                let filtered = urvim.diagnostics.get(id, "ns-a")
                let counts = urvim.diagnostics.counts(id)
                urvim.diagnostics.clear("ns-a", id)
                [all, filtered, counts, urvim.diagnostics.get(id)]
                "#,
                buffer_id.get()
            ))
            .expect("diagnostics API should set, get, count, and clear diagnostics");

        let Value::List(values) = value else {
            panic!("diagnostics result should be a list");
        };
        assert_eq!(values.len(), 4);
        let Value::List(all) = &values[0] else {
            panic!("all diagnostics should be a list");
        };
        assert_eq!(all.len(), 2);
        let Value::List(filtered) = &values[1] else {
            panic!("filtered diagnostics should be a list");
        };
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            diagnostic_field(&filtered[0], "severity"),
            Value::String("error".into())
        );
        assert_eq!(
            diagnostic_field(&filtered[0], "message"),
            Value::String("boom".into())
        );
        assert_eq!(
            diagnostic_field(&filtered[0], "source"),
            Value::String("lint-a".into())
        );
        assert_eq!(
            values[2],
            Value::Map(
                HashMap::from([
                    ("error".to_string(), Value::Number(1.0)),
                    ("warning".to_string(), Value::Number(1.0)),
                    ("info".to_string(), Value::Number(0.0)),
                    ("hint".to_string(), Value::Number(0.0)),
                ])
                .into()
            )
        );
        let Value::List(after_clear) = &values[3] else {
            panic!("remaining diagnostics should be a list");
        };
        assert_eq!(after_clear.len(), 1);

        let events = drain_editor_events();
        assert!(events.iter().any(|event| matches!(
            event,
            EditorEvent::DiagnosticsChanged { buffer_id: id } if *id == buffer_id
        )));
    }

    #[test]
    fn diagnostics_module_errors_for_invalid_range_and_severity() {
        let _guard = buffer_pool_lock();
        globals::with_diagnostics_store(|store| store.clear_all());
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        globals::set_active_buffer_id(buffer_id);
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::new(RefCell::new(layout)),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let severity = engine
            .eval(&format!(
                r#"
                urvim.diagnostics.set("ns", {}, [{{
                    "range": {{ "start": {{ "row": 0, "col": 0 }}, "end": {{ "row": 0, "col": 1 }} }},
                    "severity": "fatal",
                    "message": "boom"
                }}])
                "#,
                buffer_id.get()
            ))
            .expect_err("unknown diagnostic severity should error")
            .to_string();
        assert!(severity.contains("unknown diagnostic severity fatal"));

        let range = engine
            .eval(&format!(
                r#"
                urvim.diagnostics.set("ns", {}, [{{
                    "range": {{ "start": {{ "row": 0, "col": 0 }}, "end": {{ "row": 20, "col": 1 }} }},
                    "severity": "error",
                    "message": "boom"
                }}])
                "#,
                buffer_id.get()
            ))
            .expect_err("invalid diagnostic range should error")
            .to_string();
        assert!(range.contains("range.end row 20 col 1 is out of range"));
    }

    #[test]
    fn themes_module_lists_and_sets_active_theme() {
        let _guard = theme_registry_test_lock();
        globals::set_theme_registry(
            urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
        );
        globals::set_active_theme(
            urvim_theme::ThemeRegistry::load_builtin()
                .expect("builtins should load")
                .default_theme()
                .clone(),
        );
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(
                r#"
                let before = urvim.themes.list()
                urvim.themes.set("Nord")
                [before, urvim.themes.list()]
                "#,
            )
            .expect("themes API should list and set themes");

        let Value::List(values) = value else {
            panic!("theme result should be a list");
        };
        let Value::List(before) = &values[0] else {
            panic!("before themes should be a list");
        };
        let Value::List(after) = &values[1] else {
            panic!("after themes should be a list");
        };
        assert!(theme_entries_include(before, "Friday Night", true));
        assert!(theme_entries_include(after, "Nord", true));
        assert_eq!(
            globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string())),
            Some("Nord".to_string())
        );
    }

    #[test]
    fn themes_module_registers_and_unregisters_owned_theme() {
        let _guard = theme_registry_test_lock();
        globals::set_theme_registry(
            urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
        );
        globals::set_active_theme(
            urvim_theme::ThemeRegistry::load_builtin()
                .expect("builtins should load")
                .default_theme()
                .clone(),
        );
        let dir = std::env::temp_dir().join(format!(
            "urvim-bearscript-theme-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("theme dir should be created");
        let path = dir.join("dynamic.toml");
        std::fs::write(&path, test_theme_source("BearScript Theme"))
            .expect("theme should be written");
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(&format!(
                r#"
                let name = urvim.themes.register({:?})
                urvim.themes.set(name)
                let after_register = urvim.themes.list()
                urvim.themes.unregister(name)
                [name, after_register, urvim.themes.list()]
                "#,
                path.to_string_lossy()
            ))
            .expect("themes API should register and unregister themes");

        assert!(
            contributions
                .borrow()
                .theme("demo", "BearScript Theme")
                .is_none()
        );
        globals::with_theme_registry(|registry| {
            assert!(
                registry
                    .expect("registry should be set")
                    .get("BearScript Theme")
                    .is_none()
            );
        });
        assert_eq!(
            globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string())),
            Some("Friday Night".to_string())
        );

        let Value::List(values) = value else {
            panic!("theme result should be a list");
        };
        assert_eq!(values[0], Value::String("BearScript Theme".into()));
        let Value::List(after_register) = &values[1] else {
            panic!("registered themes should be a list");
        };
        assert!(theme_entries_include(
            after_register,
            "BearScript Theme",
            true
        ));
        let Value::List(after_unregister) = &values[2] else {
            panic!("unregistered themes should be a list");
        };
        assert!(!theme_entries_include(
            after_unregister,
            "BearScript Theme",
            false
        ));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn themes_module_creates_sets_and_unregisters_script_theme() {
        let _guard = theme_registry_test_lock();
        globals::set_theme_registry(
            urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
        );
        globals::set_active_theme(
            urvim_theme::ThemeRegistry::load_builtin()
                .expect("builtins should load")
                .default_theme()
                .clone(),
        );
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let value = engine
            .eval(&format!(
                r#"
                let name = urvim.themes.create({})
                urvim.themes.set(name)
                let after_create = urvim.themes.list()
                urvim.themes.unregister(name)
                [name, after_create, urvim.themes.list()]
                "#,
                bearscript_theme_literal("Script Theme")
            ))
            .expect("themes API should create and unregister script themes");

        assert!(
            contributions
                .borrow()
                .theme("demo", "Script Theme")
                .is_none()
        );
        globals::with_theme_registry(|registry| {
            assert!(
                registry
                    .expect("registry should be set")
                    .get("Script Theme")
                    .is_none()
            );
        });
        assert_eq!(
            globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string())),
            Some("Friday Night".to_string())
        );

        let Value::List(values) = value else {
            panic!("theme result should be a list");
        };
        assert_eq!(values[0], Value::String("Script Theme".into()));
        let Value::List(after_create) = &values[1] else {
            panic!("created themes should be a list");
        };
        assert!(theme_entries_include(after_create, "Script Theme", true));
    }

    #[test]
    fn themes_module_create_rejects_invalid_script_theme() {
        let _guard = theme_registry_test_lock();
        globals::set_theme_registry(
            urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
        );
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let unknown_field = engine
            .eval(&format!(
                r#"
                let theme = {}
                theme["unknown"] = true
                urvim.themes.create(theme)
                "#,
                bearscript_theme_literal("Invalid Field Theme")
            ))
            .expect_err("unknown theme field should error")
            .to_string();
        assert!(unknown_field.contains("unknown theme field"));

        let bad_color = engine
            .eval(
                r##"
                urvim.themes.create({
                    "name": "Bad Color Theme",
                    "palette": { "bg": -1, "fg": "#eeeeee" },
                    "default": { "fg": "fg", "bg": "bg" }
                })
                "##,
            )
            .expect_err("invalid ANSI color should error")
            .to_string();
        assert!(bad_color.contains("must be an integer from 0 to 255"));

        let bad_reference = engine
            .eval(
                r##"
                urvim.themes.create({
                    "name": "Bad Reference Theme",
                    "palette": { "bg": "#101010", "fg": "#eeeeee" },
                    "default": { "fg": "missing", "bg": "bg" }
                })
                "##,
            )
            .expect_err("unknown palette reference should error")
            .to_string();
        assert!(bad_reference.contains("unknown palette reference"));

        let duplicate = engine
            .eval(&format!(
                r#"
                urvim.themes.create({})
                urvim.themes.create({})
                "#,
                bearscript_theme_literal("Duplicate Script Theme"),
                bearscript_theme_literal("Duplicate Script Theme")
            ))
            .expect_err("duplicate script theme should error")
            .to_string();
        assert!(duplicate.contains("duplicate theme name"));
    }

    #[test]
    fn themes_module_rejects_unknown_or_unowned_theme() {
        let _guard = theme_registry_test_lock();
        globals::set_theme_registry(
            urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
        );
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let missing = engine
            .eval("urvim.themes.set(\"Not A Theme\")")
            .expect_err("unknown theme should error")
            .to_string();
        assert!(missing.contains("unknown theme"));

        let unowned = engine
            .eval("urvim.themes.unregister(\"Nord\")")
            .expect_err("unowned theme should error")
            .to_string();
        assert!(unowned.contains("does not own theme"));
    }

    #[test]
    fn ui_module_show_message_enqueues_notifications() {
        let _guard = buffer_pool_lock();
        globals::clear_notifications();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval("urvim.ui.show_message(\"hello\")")
            .expect("show_message should enqueue notifications");
        let notification = globals::active_notification(std::time::Instant::now())
            .expect("notification should be active");
        assert_eq!(notification.text, "hello");
        assert_eq!(
            notification.level,
            urvim_core::notification::NotificationLevel::Info
        );
        globals::clear_notifications();

        engine
            .eval("urvim.ui.show_message(\"careful\", { \"level\": \"warn\" })")
            .expect("show_message should enqueue warn notifications");
        let notification = globals::active_notification(std::time::Instant::now())
            .expect("notification should be active");
        assert_eq!(notification.text, "careful");
        assert_eq!(
            notification.level,
            urvim_core::notification::NotificationLevel::Warn
        );
        globals::clear_notifications();

        engine
            .eval("urvim.ui.show_message(\"boom\", { \"level\": \"error\" })")
            .expect("show_message should enqueue error notifications");
        let notification = globals::active_notification(std::time::Instant::now())
            .expect("notification should be active");
        assert_eq!(notification.text, "boom");
        assert_eq!(
            notification.level,
            urvim_core::notification::NotificationLevel::Error
        );
    }

    #[test]
    fn ui_module_show_message_errors_for_invalid_opts() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let bad_opts = engine
            .eval("urvim.ui.show_message(\"hello\", 1)")
            .expect_err("non-map opts should error")
            .to_string();
        assert!(bad_opts.contains("show_message opts must be a map or null"));

        let bad_key = engine
            .eval("urvim.ui.show_message(\"hello\", { \"timeout\": 1 })")
            .expect_err("unknown option should error")
            .to_string();
        assert!(bad_key.contains("unknown show_message option timeout"));

        let bad_level_type = engine
            .eval("urvim.ui.show_message(\"hello\", { \"level\": 1 })")
            .expect_err("non-string level should error")
            .to_string();
        assert!(bad_level_type.contains("show_message level must be a string"));

        let bad_level = engine
            .eval("urvim.ui.show_message(\"hello\", { \"level\": \"debug\" })")
            .expect_err("unknown level should error")
            .to_string();
        assert!(bad_level.contains("unknown notification level debug"));
    }

    #[test]
    fn windows_module_errors_for_unknown_window_and_invalid_cursor() {
        let _guard = buffer_pool_lock();
        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        let layout = Rc::new(RefCell::new(layout));

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                layout,
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let missing = engine
            .eval("urvim.windows.buffer(999999)")
            .expect_err("unknown window should error")
            .to_string();
        assert!(missing.contains("unknown window_id 999999"));

        let invalid_cursor = engine
            .eval("urvim.windows.set_cursor(urvim.windows.active(), 20, 0)")
            .expect_err("invalid cursor should error")
            .to_string();
        assert!(invalid_cursor.contains("cursor row 20 col 0 is out of range"));
    }

    fn drain_editor_events() -> Vec<EditorEvent> {
        let mut events = Vec::new();
        while let Some(event) = globals::take_editor_event() {
            events.push(event);
        }
        events
    }

    fn callbacks_absent_for_timer(
        runtime: &BearscriptPluginRuntime,
        plugin: &str,
        timer_id: u64,
    ) -> bool {
        !runtime
            .plugins
            .get(plugin)
            .expect("plugin should be loaded")
            .callbacks
            .borrow()
            .timers
            .contains_key(&timer_id)
    }

    fn runtime_with_fs_script(script: &str) -> BearscriptPluginRuntime {
        globals::clear_notifications();
        let fs = Rc::new(PluginFsRegistry::default());
        let mut runtime = BearscriptPluginRuntime::empty(shared_test_layout());
        runtime.fs = Rc::clone(&fs);
        let callbacks = Rc::new(RefCell::new(BearscriptPluginCallbacks::default()));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::clone(&callbacks),
                shared_test_layout(),
                fs,
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        engine
            .eval(script)
            .expect("filesystem script should evaluate");
        runtime
            .plugins
            .insert("demo".to_string(), BearscriptPlugin { engine, callbacks });
        runtime
    }

    fn dispatch_fs_until_global(
        runtime: &mut BearscriptPluginRuntime,
        global: &str,
        expected: Value,
    ) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            runtime.dispatch_fs_events();
            if let Some(plugin) = runtime.plugins.get_mut("demo") {
                let value = plugin
                    .engine
                    .eval(global)
                    .expect("filesystem result global should evaluate");
                if value == expected {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for filesystem callback global {global:?}");
    }

    fn diagnostic_field(diagnostic: &Value, field: &str) -> Value {
        let Value::Map(map) = diagnostic else {
            panic!("diagnostic should be a map");
        };
        map.get(field)
            .cloned()
            .unwrap_or_else(|| panic!("diagnostic should contain {field}"))
    }

    fn buffer_pool_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn theme_registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn test_theme_source(name: &str) -> String {
        format!(
            r##"
name = "{name}"

[palette]
bg = "#101010"
fg = "#eeeeee"

[default]
fg = "fg"
bg = "bg"
"##
        )
    }

    fn bearscript_theme_literal(name: &str) -> String {
        format!(
            r##"{{
                    "name": "{name}",
                    "palette": {{
                        "bg": "#101010",
                        "fg": "#eeeeee",
                        "accent": "#7aa2f7",
                        "muted": 244
                    }},
                    "default": {{
                        "fg": "fg",
                        "bg": "bg"
                    }},
                    "highlights": {{
                        "ui.status_bar": {{
                            "fg": "bg",
                            "bg": "accent",
                            "bold": true
                        }},
                        "syntax.comment": {{
                            "fg": "muted",
                            "italic": true
                        }}
                    }}
                }}"##
        )
    }

    #[test]
    fn ui_windows_module_creates_content_and_owned_plugin_keymaps() {
        let _guard = buffer_pool_lock();
        let layout = shared_test_layout();
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        contributions
            .borrow_mut()
            .register_command(
                "demo",
                urvim_plugin::DynamicPluginCommand {
                    name: "close".to_string(),
                    description: None,
                },
            )
            .expect("plugin command should register");

        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({
                    "placement": {
                        "type": "anchored",
                        "anchor": "top_right",
                        "margins": { "top": 1, "right": 2 }
                    },
                    "rows": 4,
                    "cols": 20,
                    "title": "Demo"
                })
                urvim.ui.windows.set_content(id, [
                    [{ "text": "hello", "style": "syntax.keyword" }]
                ])
                urvim.ui.windows.set_keymap(id, "q", "pane wrap-toggle")
                urvim.ui.windows.configure(id, {
                    "placement": {
                        "type": "anchored",
                        "anchor": "top_right",
                        "margins": { "bottom": 3, "top": null }
                    }
                })
                "#,
            )
            .expect("plugin window API should evaluate");

        let layout = layout.borrow();
        let id = layout
            .plugin_windows()
            .ids()
            .next()
            .expect("window should be created");
        let window = layout
            .plugin_windows()
            .owned_window("demo", id)
            .expect("demo should own its window");
        assert!(window.is_visible());
        assert_eq!(window.content().len(), 1);
        assert_eq!(
            window.options().placement,
            urvim_core::ui::floating_window::FloatingPlacement::Anchored {
                anchor: urvim_core::ui::floating_window::FloatingAnchor::TopRight,
                margins: urvim_core::ui::floating_window::FloatingMargins {
                    bottom: 3,
                    ..Default::default()
                },
            }
        );
        assert_eq!(
            layout
                .plugin_windows()
                .keymaps("demo", id)
                .expect("keymaps should be readable"),
            vec![(vec!["q".to_string()], "pane wrap-toggle".to_string())]
        );
    }

    #[test]
    fn ui_pickers_dynamic_items_select_original_value() {
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn selected(value) {
                urvim.ui.show_message(value)
            }
            fn init() {
                let id = urvim.ui.pickers.open({
                    "title": "Branches",
                    "on_select": selected
                })
                urvim.ui.pickers.set_items(id, [{
                    "key": "main",
                    "label": "main",
                    "detail": "origin/main",
                    "value": "selected-main"
                }])
                urvim.ui.pickers.append_items(id, [{
                    "key": "feature",
                    "label": "feature",
                    "value": "selected-feature"
                }])
            }
            "#,
        );
        let layout = Rc::clone(&runtime.layout);
        layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Tick);
        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Enter)));

        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("selection notification")
                .text,
            "selected-main"
        );
    }

    #[test]
    fn ui_pickers_escape_dispatches_cancel_callback_once() {
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn cancelled() {
                urvim.ui.show_message("cancelled")
            }
            fn selected(value) {}
            fn init() {
                urvim.ui.pickers.open({
                    "on_select": selected,
                    "on_cancel": cancelled
                })
            }
            "#,
        );
        let layout = Rc::clone(&runtime.layout);

        layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Esc)));
        assert!(runtime.dispatch_picker_events());
        assert!(!runtime.dispatch_picker_events());
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("cancel notification")
                .text,
            "cancelled"
        );
    }

    #[test]
    fn ui_confirm_returns_custom_primary_response_value() {
        let _guard = buffer_pool_lock();
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn responded(value) {
                urvim.ui.show_message(value)
            }
            fn init() {
                urvim.ui.confirm({
                    "title": "Delete",
                    "message": "Delete this file?",
                    "confirm": {
                        "label": "Delete",
                        "key": "d",
                        "value": "deleted"
                    },
                    "reject": {
                        "label": "Keep",
                        "key": "k",
                        "value": "kept"
                    },
                    "on_response": responded
                })
            }
            "#,
        );
        let layout = Rc::clone(&runtime.layout);
        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Char('d'))));

        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("response notification")
                .text,
            "deleted"
        );
        assert!(!runtime.dispatch_confirmation_events());
    }

    #[test]
    fn ui_confirm_secondary_response_is_not_cancellation() {
        let _guard = buffer_pool_lock();
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn responded(value) {
                urvim.ui.show_message(value)
            }
            fn cancelled() {
                urvim.ui.show_message("cancelled")
            }
            fn init() {
                urvim.ui.confirm({
                    "message": "Continue?",
                    "reject": { "label": "Stop", "key": "s", "value": "stopped" },
                    "on_response": responded,
                    "on_cancel": cancelled
                })
            }
            "#,
        );
        let layout = Rc::clone(&runtime.layout);
        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Char('s'))));

        crate::actions::handle_ui_result_with_shared_layout(&layout, &mut runtime, result);
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("response notification")
                .text,
            "stopped"
        );
        assert!(!runtime.dispatch_confirmation_events());
    }

    #[test]
    fn ui_confirm_escape_dispatches_cancel_callback_once() {
        let _guard = buffer_pool_lock();
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn responded(value) {}
            fn cancelled() {
                urvim.ui.show_message("cancelled")
            }
            fn init() {
                urvim.ui.confirm({
                    "message": "Continue?",
                    "on_response": responded,
                    "on_cancel": cancelled
                })
            }
            "#,
        );
        let layout = Rc::clone(&runtime.layout);

        layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Esc)));
        assert!(runtime.dispatch_confirmation_events());
        assert!(!runtime.dispatch_confirmation_events());
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("cancel notification")
                .text,
            "cancelled"
        );
    }

    #[test]
    fn ui_close_confirmation_dispatches_cancel_callback_once() {
        let _guard = buffer_pool_lock();
        globals::clear_notifications();
        let mut runtime = runtime_with_script(
            r#"
            fn responded(value) {}
            fn cancelled() {
                urvim.ui.show_message("closed")
            }
            fn init() {
                let id = urvim.ui.confirm({
                    "message": "Continue?",
                    "on_response": responded,
                    "on_cancel": cancelled
                })
                urvim.ui.close_confirmation(id)
            }
            "#,
        );

        assert!(runtime.dispatch_confirmation_events());
        assert!(!runtime.dispatch_confirmation_events());
        assert_eq!(
            globals::active_notification(Instant::now())
                .expect("cancel notification")
                .text,
            "closed"
        );
    }

    #[test]
    fn ui_panes_module_creates_targeted_pane_and_content() {
        let _guard = buffer_pool_lock();
        let layout = shared_test_layout();
        let contributions = Rc::new(RefCell::new(
            urvim_plugin::PluginContributionRegistry::default(),
        ));
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::clone(&contributions),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );
        engine
            .eval(
                r#"
                let target = urvim.windows.active()
                let id = urvim.ui.panes.create(target, {
                    "axis": "vertical",
                    "ratio": { "first": 2, "second": 1 },
                    "title": "Pane Demo"
                })
                urvim.ui.panes.set_content(id, [
                    [{ "text": "hello", "style": "syntax.keyword" }]
                ])
                urvim.ui.panes.set_keymap(id, "q", "pane close")
                "#,
            )
            .expect("plugin pane API should evaluate");

        let layout = layout.borrow();
        let id = layout
            .plugin_pane_ids("demo")
            .into_iter()
            .next()
            .expect("plugin pane should be created");
        let pane = layout
            .plugin_pane("demo", id)
            .expect("demo should own its pane");
        assert_eq!(pane.options().title.as_deref(), Some("Pane Demo"));
        assert_eq!(pane.content().len(), 1);
        assert_eq!(layout.focused_plugin_pane(), Some(id));
        assert_eq!(layout.pane_regions().len(), 2);
        assert_eq!(
            layout
                .plugin_pane_keymaps("demo", id)
                .expect("pane keymaps should be readable"),
            vec![(vec!["q".to_string()], "pane close".to_string())]
        );
    }

    #[test]
    fn ui_windows_module_rejects_invalid_content() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create()
                urvim.ui.windows.set_content(id, [
                    [{ "text": "bad\nline" }]
                ])
                "#,
            )
            .expect_err("newlines should be rejected")
            .to_string();
        assert!(error.contains("must not contain newlines"));
    }

    #[test]
    fn ui_windows_module_validates_placement() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({
                    "placement": {
                        "type": "anchored",
                        "anchor": "top_right",
                        "margins": { "diagonal": 1 }
                    }
                })
                "#,
            )
            .expect_err("unknown margin sides should be rejected")
            .to_string();
        assert!(error.contains("unknown plugin window margin diagonal"));

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({
                    "placement": {
                        "type": "anchored",
                        "anchor": "top_right",
                        "margins": { "left": -1 }
                    }
                })
                "#,
            )
            .expect_err("negative margins should be rejected")
            .to_string();
        assert!(error.contains("margins.left must be a non-negative integer or null"));

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({
                    "placement": { "type": "fixed", "row": 3, "col": 5, "margins": null }
                })
                "#,
            )
            .expect_err("fixed placement should reject margins")
            .to_string();
        assert!(error.contains("fixed placement cannot specify anchor or margins"));

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({
                    "placement": { "type": "fixed", "row": -1, "col": 5 }
                })
                "#,
            )
            .expect_err("negative fixed coordinates should be rejected")
            .to_string();
        assert!(error.contains("placement.row must be a non-negative integer"));

        let error = engine
            .eval(
                r#"
                let id = urvim.ui.windows.create({ "anchor": "center" })
                "#,
            )
            .expect_err("legacy placement fields should be rejected")
            .to_string();
        assert!(error.contains("unknown plugin window option anchor"));

        let id = engine
            .eval(
                r#"
                urvim.ui.windows.create({
                    "placement": { "type": "fixed", "row": 3, "col": 5 }
                })
                "#,
            )
            .expect("fixed placement should be accepted");
        assert!(matches!(id, Value::Number(_)));
    }

    #[test]
    fn ui_line_format_render_returns_window_compatible_content() {
        let _guard = buffer_pool_lock();
        let layout = shared_test_layout();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                Rc::clone(&layout),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        engine
            .eval(
                r#"
                let content = urvim.ui.line_format.render({
                    "width": 16,
                    "values": ["ab", "measured", "abcdef"],
                    "sections": [
                        {
                            "style": "ui.window",
                            "width": { "type": "fixed", "value": 4 },
                            "alignment": "right"
                        },
                        {
                            "style": null,
                            "width": { "type": "measured" }
                        },
                        {
                            "width": { "type": "flex", "weight": 1 },
                            "overflow": {
                                "type": "ellipsis",
                                "placement": "end"
                            }
                        }
                    ]
                })
                let id = urvim.ui.windows.create()
                urvim.ui.windows.set_content(id, content)
                "#,
            )
            .expect("formatted content should be accepted by plugin windows");

        let layout = layout.borrow();
        let id = layout
            .plugin_windows()
            .ids()
            .next()
            .expect("window should be created");
        let content = layout
            .plugin_windows()
            .owned_window("demo", id)
            .expect("demo should own the window")
            .content();

        assert_eq!(content.len(), 1);
        assert_eq!(content[0].len(), 3);
        assert_eq!(content[0][0].text, "  ab");
        assert_eq!(content[0][0].style.as_ref().unwrap().as_str(), "ui.window");
        assert_eq!(content[0][1].text, "measured");
        assert_eq!(content[0][1].style, None);
        assert_eq!(content[0][2].text, "abc…");
        assert_eq!(content[0][2].style, None);
    }

    #[test]
    fn ui_line_format_render_rejects_invalid_options() {
        let _guard = buffer_pool_lock();
        let mut engine = Engine::new();
        engine.set_global(
            "urvim",
            urvim_module(
                "demo".to_string(),
                Rc::new(RefCell::new(
                    urvim_plugin::PluginContributionRegistry::default(),
                )),
                Rc::new(RefCell::new(BearscriptPluginCallbacks::default())),
                shared_test_layout(),
                Rc::new(PluginFsRegistry::default()),
                Rc::new(PluginJobRegistry::default()),
                test_timers(),
            ),
        );

        let error = engine
            .eval(
                r#"
                urvim.ui.line_format.render({
                    "width": 10,
                    "values": ["value"],
                    "sections": [
                        {
                            "width": { "type": "flex", "weight": 0 }
                        }
                    ]
                })
                "#,
            )
            .expect_err("zero flex weights should be rejected")
            .to_string();
        assert!(error.contains("weight must be positive"));

        let error = engine
            .eval(
                r#"
                urvim.ui.line_format.render({
                    "width": 10,
                    "values": ["value"],
                    "sections": [
                        {
                            "style": "Invalid.Tag",
                            "width": { "type": "measured" }
                        }
                    ]
                })
                "#,
            )
            .expect_err("invalid theme tags should be rejected")
            .to_string();
        assert!(error.contains("sections[0].style is invalid"));
    }

    #[test]
    fn emoji_picker_example_inserts_selected_emoji_at_cursor() {
        let _guard = buffer_pool_lock();
        let plugin_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/emoji-picker");
        let plugin_config = std::collections::BTreeMap::from([(
            "emoji-picker".to_string(),
            urvim_plugin::PluginConfigEntry {
                enabled: true,
                path: plugin_root,
            },
        )]);
        let registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
            .expect("emoji picker registry should load");
        let layout = Rc::new(RefCell::new(Layout::new(WindowGroup::from_buffers(vec![
            Buffer::from_str("x"),
        ]))));
        let buffer_id = layout.borrow().active_buffer_view().buffer_id();
        let plugin = registry
            .get("emoji-picker")
            .expect("emoji picker plugin should be present");
        let mut runtime = BearscriptPluginRuntime::empty(Rc::clone(&layout));
        runtime
            .load_plugin("emoji-picker", plugin)
            .expect("emoji picker plugin should load");

        runtime
            .run_command("emoji-picker", "open", &[])
            .expect("emoji picker command should run");
        assert!(layout.borrow().plugin_picker_is_open());
        layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Tick);
        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Enter)));
        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));

        assert_eq!(
            globals::with_buffer(buffer_id, |buffer| buffer.as_str()),
            Some("😀x".to_string())
        );
        assert_eq!(
            layout.borrow().active_buffer_view().cursor(),
            Cursor::new(0, 4)
        );
    }

    #[test]
    fn window_demo_example_loads_and_creates_a_focused_window() {
        let _guard = buffer_pool_lock();
        let plugin_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/window-demo");
        let plugin_config = std::collections::BTreeMap::from([(
            "window-demo".to_string(),
            urvim_plugin::PluginConfigEntry {
                enabled: true,
                path: plugin_root,
            },
        )]);
        let registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
            .expect("window demo registry should load");
        urvim_core::command::install_configured_commands_with_plugins(
            &urvim_core::config::Config::default(),
            &registry,
        )
        .expect("window demo command namespace should install");

        let layout = shared_test_layout();
        let plugin = registry
            .get("window-demo")
            .expect("window demo plugin should be present");
        let mut runtime = BearscriptPluginRuntime::empty(Rc::clone(&layout));
        runtime
            .load_plugin("window-demo", &plugin)
            .expect("window demo plugin should load");

        let layout = layout.borrow();
        let id = layout
            .plugin_windows()
            .ids()
            .next()
            .expect("window demo should create a window");
        assert_eq!(layout.plugin_windows().focused(), Some(id));
        let content = layout
            .plugin_windows()
            .owned_window("window-demo", id)
            .unwrap()
            .content();
        assert_eq!(content.len(), 13);
        assert_eq!(
            content[0][0].style.as_ref().unwrap().as_str(),
            "syntax.keyword"
        );
        assert_eq!(
            content[0][1].style.as_ref().unwrap().as_str(),
            "syntax.type"
        );
        assert_eq!(
            content[6][0].style.as_ref().unwrap().as_str(),
            "syntax.constant"
        );
        assert_eq!(
            content[12][0].style.as_ref().unwrap().as_str(),
            "syntax.string"
        );
    }

    #[test]
    fn window_demo_toggles_between_floating_and_docked_representations() {
        let _guard = buffer_pool_lock();
        let plugin_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/window-demo");
        let plugin_config = std::collections::BTreeMap::from([(
            "window-demo".to_string(),
            urvim_plugin::PluginConfigEntry {
                enabled: true,
                path: plugin_root,
            },
        )]);
        let registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
            .expect("window demo registry should load");
        urvim_core::command::install_configured_commands_with_plugins(
            &urvim_core::config::Config::default(),
            &registry,
        )
        .expect("window demo command namespace should install");

        let layout = shared_test_layout();
        let plugin = registry
            .get("window-demo")
            .expect("window demo plugin should be present");
        let mut runtime = BearscriptPluginRuntime::empty(Rc::clone(&layout));
        runtime
            .load_plugin("window-demo", plugin)
            .expect("window demo plugin should load");

        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Char('d'))));
        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));
        {
            let layout = layout.borrow();
            assert!(layout.plugin_windows().ids().next().is_none());
            assert_eq!(layout.plugin_pane_ids("window-demo").len(), 1);
            assert!(layout.focused_plugin_pane().is_some());
        }

        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Char('d'))));
        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));
        let layout = layout.borrow();
        assert_eq!(layout.plugin_pane_ids("window-demo").len(), 0);
        assert_eq!(layout.plugin_windows().ids().count(), 1);
        assert!(layout.plugin_windows().focused().is_some());
    }

    #[test]
    fn focused_plugin_window_command_can_mutate_layout_without_reentrant_borrow() {
        let _guard = buffer_pool_lock();
        let plugin_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/window-demo");
        let plugin_config = std::collections::BTreeMap::from([(
            "window-demo".to_string(),
            urvim_plugin::PluginConfigEntry {
                enabled: true,
                path: plugin_root,
            },
        )]);
        let registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
            .expect("window demo registry should load");
        urvim_core::command::install_configured_commands_with_plugins(
            &urvim_core::config::Config::default(),
            &registry,
        )
        .expect("window demo command namespace should install");

        let layout = shared_test_layout();
        let plugin = registry
            .get("window-demo")
            .expect("window demo plugin should be present");
        let mut runtime = BearscriptPluginRuntime::empty(Rc::clone(&layout));
        runtime
            .load_plugin("window-demo", plugin)
            .expect("window demo plugin should load");

        let result = layout
            .borrow_mut()
            .route_ui_event(&urvim_core::ui::UiEvent::Key(Key::new(KeyCode::Char('l'))));
        assert!(crate::actions::handle_ui_result_with_shared_layout(
            &layout,
            &mut runtime,
            result,
        ));

        let layout = layout.borrow();
        let id = layout
            .plugin_windows()
            .focused()
            .expect("window should remain focused");
        assert!(matches!(
            layout
                .plugin_windows()
                .owned_window("window-demo", id)
                .expect("demo should own its window")
                .options()
                .placement,
            urvim_core::ui::floating_window::FloatingPlacement::Anchored {
                anchor: urvim_core::ui::floating_window::FloatingAnchor::TopRight,
                ..
            }
        ));
    }

    fn theme_entries_include(entries: &[Value], name: &str, active: bool) -> bool {
        entries.iter().any(|entry| {
            let Value::Map(entry) = entry else {
                return false;
            };
            entry.get("name") == Some(&Value::String(name.to_string().into_boxed_str().into()))
                && entry.get("active") == Some(&Value::Bool(active))
        })
    }
}
