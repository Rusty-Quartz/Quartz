#[derive(Hash, PartialEq, Eq, Clone)]
pub enum Listeners {
    StatusResponse,
    Pong,
    Disconnect,
    EncryptionRequest,
    LoginSuccess,
    SetCompression,
    LoginPluginRequest,
}

pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub listeners: Vec<Listeners>,
    pub quartz_data: PluginMetadata,
}

pub fn get_quartz_info() -> PluginMetadata {
    PluginMetadata {
        quartz_version: 1,
        breaking_version: 0,
    }
}

pub struct PluginMetadata {
    pub quartz_version: u8,
    pub breaking_version: u8,
}
