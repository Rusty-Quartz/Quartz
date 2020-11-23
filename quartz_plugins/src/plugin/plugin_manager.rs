use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::fs::{create_dir, read_dir};
use std::path::Path;
use std::sync::Arc;

use log::*;

use crate::plugin::plugin_info::{Listeners, PluginInfo};
pub struct PluginManager {
    pub plugins: Vec<PluginInfo>,
    listeners: HashMap<Listeners, Vec<Arc<Library>>>,
}

impl PluginManager {
    pub fn new(plugin_folder: &Path) -> PluginManager {
        let plugin_files = match read_dir(plugin_folder) {
            Ok(val) => val,
            Err(_) => {
                // If we can't create the directory then we can panic
                create_dir(plugin_folder).unwrap();
                read_dir(plugin_folder).unwrap()
            }
        };

        let mut plugins: Vec<PluginInfo> = Vec::new();
        let mut listeners: HashMap<Listeners, Vec<Arc<Library>>> = HashMap::new();

        for file in plugin_files {
            // I don't really see how this could fail
            let path = file.unwrap().path();

            if path.is_file() {
                let plugin = Arc::new(match Library::new(path.clone()) {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Error loading plugin file {}, skipping it", e);
                        continue;
                    }
                });

                let plugin_info: PluginInfo;

                // This is increadibly horribly unsafe but we're going to assume plugins are fine because idk any way to make sure they're safe
                unsafe {
                    let func: Symbol<unsafe extern "C" fn() -> PluginInfo> =
                        match plugin.get(b"get_plugin_info") {
                            Ok(f) => f,
                            Err(_e) => {
                                error!(
                                "plugin {:?} doesn't have a get_plugin_info function, skipping it",
                                path
                            );
                                continue;
                            }
                        };

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

        PluginManager { plugins, listeners }
    }

    pub fn get_listeners(&self, key: Listeners) -> Option<Vec<Arc<Library>>> {
        let listeners = self.listeners.get(&key);
        if listeners.is_some() {
            Some(listeners.unwrap().to_owned())
        } else {
            None
        }
    }

    pub fn run_listeners<T: Listenable>(&self, key: Listeners, start: T, method_name: String) -> T {
        let listeners = self.get_listeners(key);
        if listeners.is_none() {
            return start;
        }

        let mut this = start;
        for plugin in listeners.unwrap() {
            unsafe {
                let func: Symbol<unsafe extern "C" fn(input: T) -> T> =
                    plugin.get(method_name.as_bytes()).unwrap();
                this = func(this);
            }
        }
        this
    }
}

pub trait Listenable {
    fn run_listeners(self, manager: &PluginManager) -> Self;
}
