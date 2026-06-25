use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;

/// Creates the `urvim.filetypes` BearScript module.
pub(in crate::plugin) fn filetypes_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
) -> Value {
    let register_plugin = plugin.clone();
    let register_contributions = Rc::clone(&contributions);
    let detect_contributions = Rc::clone(&contributions);
    Value::Module(
        HashMap::from([
            (
                "register".to_string(),
                super::super::native_fn(
                    "filetypes.register",
                    move |name: String, _opts: Option<Value>| {
                        register_contributions.borrow_mut().register_filetype(
                            register_plugin.clone(),
                            urvim_plugin::DynamicFiletype { name },
                        )?;
                        Ok(())
                    },
                ),
            ),
            (
                "detect_extension".to_string(),
                super::super::native_fn(
                    "filetypes.detect_extension",
                    move |extension: String, filetype: String| {
                        detect_contributions
                            .borrow_mut()
                            .detect_filetype_extension(extension, filetype)?;
                        Ok(())
                    },
                ),
            ),
        ])
        .into(),
    )
}
