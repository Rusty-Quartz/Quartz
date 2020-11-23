pub mod plugin {
    pub mod plugin_info;
    pub mod plugin_manager;
}

pub use plugin::plugin_info::Listeners;
pub use plugin::plugin_info::PluginInfo;
pub use plugin::plugin_manager::Listenable;
pub use plugin::plugin_manager::PluginManager;

pub static PLUGIN_VERSION: u8 = 1;
