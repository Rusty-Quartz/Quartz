use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::fs::read_dir;
use std::path::Path;
use std::sync::Arc;

use log::*;

use crate::plugin::plugin_info::{Listeners, PluginInfo};
pub struct PluginManager {
    plugins: Vec<PluginInfo>,
    listeners: HashMap<Listeners, Vec<Arc<Library>>>
}

impl PluginManager {
    pub fn new(plugin_folder: &Path) -> std::io::Result<PluginManager> {
        let plugin_files = read_dir(plugin_folder)?;

        let mut plugins: Vec<PluginInfo> = Vec::new();
        let mut listeners: HashMap<Listeners, Vec<Arc<Library>>> = HashMap::new();

        for file in plugin_files {
            let file = file?;
            let path = file.path();

            if path.is_file() {
                let plugin = Arc::new(Library::new(path).unwrap());
                let plugin_info: PluginInfo;
                unsafe {
                    let func: Symbol<unsafe extern fn() -> PluginInfo> = plugin.get(b"get_plugin_info").unwrap();
                    plugin_info = func();
                }
                info!("Loading plugin: {}", &plugin_info.name);
                
                for listener in &plugin_info.listeners {
                    if listeners.contains_key(listener) {
                        listeners.get_mut(listener).unwrap().push(plugin.clone());
                    } else {
                        listeners.insert(listener.clone(), vec![plugin.clone()]);
                    }
                }
                plugins.push(plugin_info);
            }
        }

        Ok(PluginManager {
            plugins,
            listeners
        })
    }
}

