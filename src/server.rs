use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, Duration};

use futures::channel::mpsc::{UnboundedSender, UnboundedReceiver};

use linefeed::{Interface, DefaultTerminal};
use linefeed::ReadResult;

use log::*;

use crate::config::Config;
use crate::network::packet_handler::{
    WrappedServerPacket,
    ServerBoundPacket,
    ClientBoundPacket,
    dispatch_sync_packet
};
use crate::network::connection::WriteHandle;
use crate::util::ioutil::ByteBuffer;
use crate::command::executor::*;
use crate::{unchecked_component, custom_color};

pub const VERSION: &str = "1.15.2";

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

pub struct QuartzServer<'a> {
    pub config: Config,
    pub client_list: ClientList,
    pub console_interface: Arc<Interface<DefaultTerminal>>,
    pub read_stdin: Arc<AtomicBool>,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
    join_handles: HashMap<String, JoinHandle<()>>,
    pub command_executor: CommandExecutor<'a>,
    pub clock: ServerClock
}

impl<'a> QuartzServer<'a> {
    pub fn new(
        config: Config,
        sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
        console_interface: Arc<Interface<DefaultTerminal>>
    ) -> Self {
        if RUNNING.compare_and_swap(false, true, Ordering::SeqCst) {
            panic!("Attempted to create a server instance after one was already created.");
        }

        QuartzServer {
            config,
            client_list: ClientList::new(),
            console_interface,
            read_stdin: Arc::new(AtomicBool::new(true)),
            sync_packet_receiver,
            join_handles: HashMap::new(),
            command_executor: CommandExecutor::new(),
            clock: ServerClock::new(50)
        }
    }

    pub fn init(&mut self, command_pipe: UnboundedSender<WrappedServerPacket>) {
        self.command_executor.register("stop", executor(|_ctx| {
            RUNNING.compare_and_swap(true, false, Ordering::SeqCst);
        }));

        self.command_executor.register("tps", executor(|ctx| {
            let mspt = ctx.server.clock.mspt();
            let tps = ctx.server.clock.as_tps(mspt);
            let red: f32;
            let green: f32;

            // Shift from dark green to yellow
            if tps > 15.0 {
                green = 128.0 + 14.4 * (20.0 - tps);
                red = 40.0 * (20.0 - tps);
            }
            // Shift from yellow to light red
            else if tps > 10.0 {
                green = 200.0 - 40.0 * (15.0 - tps);
                red = 200.0 + 11.0 * (15.0 - tps);
            }
            // Shift from light red to dark red
            else if tps > 0.0 {
                green = 0.0;
                red = 255.0 - 15.5 * (10.0 - tps);
            }
            // If everything is working this should never run
            else {
                green = 128.0;
                red = 0.0;
            }

            ctx.sender.send_message(unchecked_component!(
                "&(gold)Server TPS: &({}){:.2} ({:.3} mspt)",
                custom_color!(red, green, 0),
                tps,
                mspt
            ));
        }));

        // Setup the command handler thread
        let interface = self.console_interface.clone();
        let read_stdin = self.read_stdin.clone();
        self.add_join_handle("Console Command Reader", thread::spawn(move || {
            while RUNNING.load(Ordering::Relaxed) {
                // Check for a new command every 50ms
                match interface.read_line_step(Some(Duration::from_millis(50))) {
                    Ok(result) => match result {
                        Some(ReadResult::Input(command)) => {
                            // Disable console input until the server re-enables it
                            read_stdin.store(false, Ordering::SeqCst);

                            interface.add_history_unique(command.clone());

                            // Forward the command to the server thread
                            let packet = WrappedServerPacket::new(0, ServerBoundPacket::HandleConsoleCommand {
                                command: command.trim().to_owned()
                            });
                            if let Err(e) = command_pipe.unbounded_send(packet) {
                                error!("Failed to forward console command to server thread: {}", e);
                            }
                        },
                        _ => {}
                    },
                    Err(e) => error!("Failed to read console input: {}", e)
                }

                // Wait for stdin reading to be re-enabled
                while !read_stdin.load(Ordering::SeqCst) {}
            }
        }));
    }

    pub fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }

    pub fn run(&mut self) {
        info!("Started server thread");

        while RUNNING.load(Ordering::Relaxed) {
            self.clock.start();
            self.tick();
            self.clock.finish_tick();
        }
    }

    fn tick(&mut self) {
        self.handle_packets();
    }

    fn handle_packets(&mut self) {
        while let Ok(packet_wrapper) = self.sync_packet_receiver.try_next() {
            match packet_wrapper {
                Some(packet) => {
                    dispatch_sync_packet(&packet, &mut *self);
                },
                None => break
            }
        }
    }
}

impl<'a> Drop for QuartzServer<'a> {
    fn drop(&mut self) {
        for (thread_name, handle) in self.join_handles.drain() {
            info!("Shutting down {}", thread_name);
            if let Err(_) = handle.join() {
                error!("Failed to join {}", thread_name);
            }
        }
    }
}

pub struct ServerClock {
    micros_readings: [u32; 100],
    micros_index: usize,
    full_tick_millis: u128,
    full_tick: Duration,
    time: SystemTime
}

impl ServerClock {
    pub fn new(tick_length: u128) -> Self {
        ServerClock {
            micros_readings: [0_u32; 100],
            micros_index: 0,
            full_tick_millis: tick_length,
            full_tick: Duration::from_millis(tick_length as u64),
            time: SystemTime::now()
        }
    }

    pub fn start(&mut self) {
        self.time = SystemTime::now();
    }

    pub fn finish_tick(&mut self) {
        match self.time.elapsed() {
            Ok(duration) => {
                self.micros_readings[self.micros_index] = duration.as_micros() as u32;
                self.micros_index = (self.micros_index + 1) % 100;

                if duration.as_millis() < self.full_tick_millis {
                    thread::sleep(self.full_tick - duration);
                }
            },
            Err(_) => thread::sleep(self.full_tick)
        }
    }

    pub fn mspt(&self) -> f32 {
        let mut sum: f32 = 0.0;
        for i in 0..100 {
            sum += self.micros_readings[i] as f32;
        }
        sum / 100_000_f32
    }

    pub fn as_tps(&self, mspt: f32) -> f32 {
        if mspt < self.full_tick_millis as f32 {
            1000_f32 / (self.full_tick_millis as f32)
        } else {
            1000_f32 / mspt
        }
    }
}

#[repr(transparent)]
pub struct ClientList(Arc<Mutex<HashMap<usize, Client>>>);

impl ClientList {
    pub fn new() -> Self {
        ClientList(Arc::new(Mutex::new(HashMap::new())))
    }

    fn lock(&self) -> MutexGuard<HashMap<usize, Client>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(_) => panic!("Client list mutex poisoned.")
        }
    }

    pub fn add_client(&self, client_id: usize, connection: WriteHandle) {
        self.lock().insert(client_id, Client::new(connection));
    }

    pub fn remove_client(&self, client_id: usize) {
        self.lock().remove(&client_id);
    }

    pub fn online_count(&self) -> usize {
        self.lock().iter().map(|(_id, client)| client.player_id).flatten().count()
    }

    pub fn send_packet(&self, client_id: usize, packet: &ClientBoundPacket) {
        match self.lock().get_mut(&client_id) {
            Some(client) => client.connection.send_packet(packet),
            None => warn!("Attempted to send packet to disconnected client.")
        }
    }

    pub fn send_buffer(&self, client_id: usize, buffer: &ByteBuffer) {
        match self.lock().get_mut(&client_id) {
            Some(client) => client.connection.send_buffer(buffer),
            None => warn!("Attempted to send buffer to disconnected client.")
        }
    }
}

impl Clone for ClientList {
    fn clone(&self) -> Self {
        ClientList(self.0.clone())
    }
}

struct Client {
    pub connection: WriteHandle,
    pub player_id: Option<usize>
}

impl Client {
    pub fn new(connection: WriteHandle) -> Self {
        Client {
            connection,
            player_id: None
        }
    }
}