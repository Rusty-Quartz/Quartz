use quartz::{
    Listeners,
    get_quartz_info,
    network::{
        self,
        ClientBoundPacket
    }
};
use serde_json::json;

#[no_mangle]
pub fn get_plugin_info() -> quartz::PluginInfo {
    quartz::PluginInfo {
        name: "Test plugin".to_owned(),
        version: "1.0.0".to_owned(),
        listeners: vec![Listeners::StatusResponse],
        quartz_data: get_quartz_info()
    }
}


#[no_mangle]
pub fn on_status_response(packet: &mut ClientBoundPacket) -> ClientBoundPacket {
    // packet should always be StatusResponsePacket but checking never hurts
    match packet {
        ClientBoundPacket::StatusResponse {..} => {
            println!("Plugin is running :coolchamp:");
            ClientBoundPacket::StatusResponse {
                json_response: json!({
                    "version": {
                        "name": "1.17 lel",
                        "protocol": network::PROTOCOL_VERSION
                    },
                    "players": {
                        "max": 9001,
                        "online": 0,
                        "sample": [] // Maybe implement this in the future
                    },
                    "description": "Server has been hijacked by a plugin"
                }).to_string()
            }
        }
        _ => {
            panic!("Something has gone horribly wrong");
        }
    }
}