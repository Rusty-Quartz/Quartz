use futures::channel::mpsc::UnboundedReceiver;
use crate::config::Config;
use crate::network::packet_handler::WrappedServerPacket;

static mut SERVER_INSTANCE: Option<QuartzServer> = None;

pub fn init_server(
    config: Config,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>
) -> &'static QuartzServer {
    unsafe {
        match SERVER_INSTANCE {
            Some(_) => panic!("Attempted to initialize server after init_server was already called."),
            None => {
                SERVER_INSTANCE = Some(QuartzServer {
                    config,
                    sync_packet_receiver,
                    running: true,
                    version: "1.15.2"
                });

                SERVER_INSTANCE.as_ref().unwrap()
            }
        }
    }
}

#[inline]
pub fn get_server() -> &'static QuartzServer {
    unsafe {
        match &SERVER_INSTANCE {
            Some(server) => server,
            None => panic!("Attempted to access server instance before it was initialized.")
        }
    }
}

pub struct QuartzServer {
    pub config: Config,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
    pub running: bool,
    pub version: &'static str
}