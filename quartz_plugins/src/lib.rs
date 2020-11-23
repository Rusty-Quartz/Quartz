pub mod plugin {
    pub mod plugin_info;
    pub mod plugin_manager;
}

pub use plugin::{
    plugin_info::{Listeners, PluginInfo},
    plugin_manager::{Listenable, PluginManager},
};

pub static PLUGIN_VERSION: u8 = 1;
