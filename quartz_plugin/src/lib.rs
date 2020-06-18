pub mod plugin {
    pub mod plugin_manager;
    pub mod plugin_info;
}

pub use plugin_manager::PluginManager;


pub static PLUGIN_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
