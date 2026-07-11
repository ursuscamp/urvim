use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::{IntoFunction, Value};

mod env;
mod filetypes;
mod fs;
mod inspect;
mod json;
mod keymaps;
mod lists;
mod path;
mod project;
mod registers;
mod strings;
mod syntax;
mod themes;
mod ui;

use super::callbacks::BearscriptPluginCallbacks;
use super::fs::PluginFsRegistry;
use super::jobs::{PluginJobRegistry, job_id_from_number};
use super::timers::{PluginTimerRegistry, timer_id_from_number, timer_ms_from_number};
use super::{
    SharedLayout, buffers_module, command_execute_fn, diagnostics_module, event_constants,
    selection_module, windows_module,
};
use env::env_module;
use filetypes::filetypes_module;
use fs::fs_module;
use inspect::inspect_fn;
use json::json_module;
use keymaps::keymaps_module;
use lists::lists_module;
use path::path_module;
use project::project_module;
use registers::registers_module;
use strings::strings_module;
use syntax::syntax_module;
use themes::themes_module;
use ui::ui_module;

pub(in crate::plugin) fn urvim_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    layout: SharedLayout,
    fs: Rc<PluginFsRegistry>,
    jobs: Rc<PluginJobRegistry>,
    timers: Rc<PluginTimerRegistry>,
) -> Value {
    UrvimModuleBuilder {
        plugin,
        contributions,
        callbacks,
        layout,
        fs,
        jobs,
        timers,
    }
    .build()
}

struct UrvimModuleBuilder {
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    layout: SharedLayout,
    fs: Rc<PluginFsRegistry>,
    jobs: Rc<PluginJobRegistry>,
    timers: Rc<PluginTimerRegistry>,
}

impl UrvimModuleBuilder {
    fn build(self) -> Value {
        let mut module = HashMap::new();
        module.insert("events".to_string(), event_constants());
        module.insert("buffers".to_string(), buffers_module());
        module.insert(
            "windows".to_string(),
            windows_module(Rc::clone(&self.layout)),
        );
        module.insert(
            "selection".to_string(),
            selection_module(Rc::clone(&self.layout)),
        );
        module.insert("registers".to_string(), registers_module());
        module.insert("commands".to_string(), self.commands_module());
        module.insert("keymaps".to_string(), keymaps_module());
        module.insert("diagnostics".to_string(), diagnostics_module());
        module.insert(
            "themes".to_string(),
            themes_module(self.plugin.clone(), Rc::clone(&self.contributions)),
        );
        module.insert(
            "ui".to_string(),
            ui_module(
                self.plugin.clone(),
                Rc::clone(&self.contributions),
                Rc::clone(&self.layout),
            ),
        );
        module.insert("strings".to_string(), strings_module());
        module.insert("path".to_string(), path_module());
        module.insert(
            "fs".to_string(),
            fs_module(
                self.plugin.clone(),
                Rc::clone(&self.fs),
                Rc::clone(&self.callbacks),
            ),
        );
        module.insert("env".to_string(), env_module());
        module.insert(
            "filetypes".to_string(),
            filetypes_module(self.plugin.clone(), Rc::clone(&self.contributions)),
        );
        module.insert("json".to_string(), json_module());
        module.insert("lists".to_string(), lists_module());
        module.insert("project".to_string(), project_module());
        module.insert("jobs".to_string(), self.jobs_module());
        module.insert("timers".to_string(), self.timers_module());
        module.insert(
            "syntax".to_string(),
            syntax_module(
                self.plugin.clone(),
                Rc::clone(&self.contributions),
                Rc::clone(&self.callbacks),
            ),
        );
        module.insert("inspect".to_string(), inspect_fn());
        module.insert(
            "command".to_string(),
            command_execute_fn("command", Rc::clone(&self.layout)),
        );
        module.extend(self.event_hook_api());
        Value::Module(module.into())
    }

    fn commands_module(&self) -> Value {
        let register_plugin = self.plugin.clone();
        let register_contributions = Rc::clone(&self.contributions);
        let register_callbacks = Rc::clone(&self.callbacks);
        let unregister_plugin = self.plugin.clone();
        let unregister_contributions = Rc::clone(&self.contributions);
        let unregister_callbacks = Rc::clone(&self.callbacks);
        let list_plugin = self.plugin.clone();
        let list_contributions = Rc::clone(&self.contributions);
        let execute_layout = Rc::clone(&self.layout);
        Value::Module(
            HashMap::from([
                (
                    "register".to_string(),
                    super::native_fn(
                        "commands.register",
                        move |name: String, callback: Value, description: Option<String>| {
                            super::register_plugin_command(
                                &register_plugin,
                                Rc::clone(&register_contributions),
                                Rc::clone(&register_callbacks),
                                name,
                                callback,
                                description,
                            )
                        },
                    ),
                ),
                (
                    "unregister".to_string(),
                    super::native_fn("commands.unregister", move |name: String| {
                        super::unregister_plugin_command(
                            &unregister_plugin,
                            Rc::clone(&unregister_contributions),
                            Rc::clone(&unregister_callbacks),
                            &name,
                        );
                        Ok(())
                    }),
                ),
                (
                    "list".to_string(),
                    super::native_fn("commands.list", move || {
                        let commands: Vec<_> = list_contributions
                            .borrow()
                            .commands(&list_plugin)
                            .map(super::command_to_value)
                            .collect();
                        Ok(Value::List(commands.into()))
                    }),
                ),
                (
                    "execute".to_string(),
                    command_execute_fn("commands.execute", execute_layout),
                ),
            ])
            .into(),
        )
    }

    fn event_hook_api(&self) -> HashMap<String, Value> {
        let mut module = HashMap::new();
        let hook_plugin = self.plugin.clone();
        let hook_contributions = Rc::clone(&self.contributions);
        let hook_callbacks = Rc::clone(&self.callbacks);
        module.insert(
            "register_event_hook".to_string(),
            super::native_fn(
                "register_event_hook",
                move |event: String, callback: Value| {
                    super::validate_callback(&callback, "event hook callback")?;
                    let event = event.parse::<urvim_plugin::PluginEventKind>()?;
                    let hook_id = {
                        let mut callbacks = hook_callbacks.borrow_mut();
                        let hook_id = callbacks.next_hook_id;
                        callbacks.next_hook_id += 1;
                        callbacks.event_hooks.insert(hook_id, callback);
                        hook_id
                    };
                    hook_contributions.borrow_mut().register_event_hook(
                        hook_plugin.clone(),
                        event,
                        hook_id,
                    )?;
                    Ok(hook_id as f64)
                },
            ),
        );

        let unhook_plugin = self.plugin.clone();
        let unhook_contributions = Rc::clone(&self.contributions);
        let unhook_callbacks = Rc::clone(&self.callbacks);
        module.insert(
            "unregister_event_hook".to_string(),
            super::native_fn("unregister_event_hook", move |hook_id: f64| {
                let hook_id = super::hook_id_from_number(hook_id)?;
                unhook_contributions
                    .borrow_mut()
                    .unregister_event_hook(&unhook_plugin, hook_id);
                unhook_callbacks.borrow_mut().event_hooks.remove(&hook_id);
                Ok(())
            }),
        );
        module
    }

    fn jobs_module(&self) -> Value {
        let spawn_plugin = self.plugin.clone();
        let spawn_jobs = Rc::clone(&self.jobs);
        let spawn_callbacks = Rc::clone(&self.callbacks);
        let kill_jobs = Rc::clone(&self.jobs);
        let status_jobs = Rc::clone(&self.jobs);
        let write_jobs = Rc::clone(&self.jobs);
        let close_jobs = Rc::clone(&self.jobs);
        let list_jobs = Rc::clone(&self.jobs);
        Value::Module(
            HashMap::from([
                (
                    "spawn".to_string(),
                    super::native_fn("jobs.spawn", move |opts: Value| {
                        let spawn = spawn_jobs.spawn(&spawn_plugin, opts)?;
                        spawn_callbacks
                            .borrow_mut()
                            .jobs
                            .insert(spawn.id, spawn.callbacks);
                        Ok(spawn.id as f64)
                    }),
                ),
                (
                    "kill".to_string(),
                    super::native_fn("jobs.kill", move |job_id: f64| {
                        kill_jobs.kill(job_id_from_number(job_id)?)
                    }),
                ),
                (
                    "status".to_string(),
                    super::native_fn("jobs.status", move |job_id: f64| {
                        Ok(status_jobs
                            .status(job_id_from_number(job_id)?)?
                            .as_str()
                            .to_string())
                    }),
                ),
                (
                    "write_stdin".to_string(),
                    super::native_fn("jobs.write_stdin", move |job_id: f64, text: String| {
                        write_jobs.write_stdin(job_id_from_number(job_id)?, &text)
                    }),
                ),
                (
                    "close_stdin".to_string(),
                    super::native_fn("jobs.close_stdin", move |job_id: f64| {
                        close_jobs.close_stdin(job_id_from_number(job_id)?)
                    }),
                ),
                (
                    "list".to_string(),
                    super::native_fn("jobs.list", move || {
                        Ok(Value::List(list_jobs.list().into()))
                    }),
                ),
            ])
            .into(),
        )
    }

    fn timers_module(&self) -> Value {
        let defer_plugin = self.plugin.clone();
        let defer_timers = Rc::clone(&self.timers);
        let defer_callbacks = Rc::clone(&self.callbacks);
        let timeout_plugin = self.plugin.clone();
        let timeout_timers = Rc::clone(&self.timers);
        let timeout_callbacks = Rc::clone(&self.callbacks);
        let interval_plugin = self.plugin.clone();
        let interval_timers = Rc::clone(&self.timers);
        let interval_callbacks = Rc::clone(&self.callbacks);
        let clear_timers = Rc::clone(&self.timers);
        let clear_callbacks = Rc::clone(&self.callbacks);
        Value::Module(
            HashMap::from([
                (
                    "defer".to_string(),
                    super::native_fn("timers.defer", move |callback: Value| {
                        super::validate_callback(&callback, "timer callback")?;
                        let id = defer_timers.defer(&defer_plugin);
                        defer_callbacks.borrow_mut().timers.insert(id, callback);
                        Ok(id as f64)
                    }),
                ),
                (
                    "set_timeout".to_string(),
                    super::native_fn("timers.set_timeout", move |ms: f64, callback: Value| {
                        super::validate_callback(&callback, "timer callback")?;
                        let id =
                            timeout_timers.set_timeout(&timeout_plugin, timer_ms_from_number(ms)?);
                        timeout_callbacks.borrow_mut().timers.insert(id, callback);
                        Ok(id as f64)
                    }),
                ),
                (
                    "set_interval".to_string(),
                    super::native_fn("timers.set_interval", move |ms: f64, callback: Value| {
                        super::validate_callback(&callback, "timer callback")?;
                        let id = interval_timers
                            .set_interval(&interval_plugin, timer_ms_from_number(ms)?);
                        interval_callbacks.borrow_mut().timers.insert(id, callback);
                        Ok(id as f64)
                    }),
                ),
                (
                    "clear".to_string(),
                    super::native_fn("timers.clear", move |timer_id: f64| {
                        let timer_id = timer_id_from_number(timer_id)?;
                        clear_timers.clear(timer_id);
                        clear_callbacks.borrow_mut().timers.remove(&timer_id);
                        Ok(())
                    }),
                ),
            ])
            .into(),
        )
    }
}

pub(in crate::plugin) fn native_fn<F, Args>(name: &str, f: F) -> Value
where
    F: IntoFunction<Args>,
{
    let mut function = f.into_function();
    function.name = name.to_string();
    Value::NativeFn(function)
}
