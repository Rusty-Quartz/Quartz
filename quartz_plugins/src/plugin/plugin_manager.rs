use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::fs::{read_dir, create_dir};
use std::path::Path;
use std::sync::Arc;

use log::*;

use crate::plugin::plugin_info::{Listeners, PluginInfo};
pub struct PluginManager {
    pub plugins: Vec<PluginInfo>,
    listeners: HashMap<Listeners, Vec<Arc<Library>>>
}

impl PluginManager {
    pub fn new(plugin_folder: &Path) -> std::io::Result<PluginManager> {
        let plugin_files = match read_dir(plugin_folder) {
            Ok(val) => val,
            Err(_) => {
                create_dir(plugin_folder)?;
                read_dir(plugin_folder)?
            }
        };

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

    pub fn get_listeners(&self, key: Listeners) -> Vec<Arc<Library>> {
        self.listeners.get(&key).unwrap().to_owned()
    }

    pub fn run_listeners<T: Listenable>(&self, key: Listeners, start: T, method_name: String) -> T {
        let mut this = start;
        for plugin in self.get_listeners(key) {
            unsafe {
                let func: Symbol<unsafe extern fn(input: T) -> T> = plugin.get(method_name.as_bytes()).unwrap();
                this = func(this);
            }
        }
        this
    }
}

pub trait Listenable {
    fn run_listeners(self, manager: &PluginManager) -> Self;
}