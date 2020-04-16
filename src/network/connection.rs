use std::io::Result;
use std::io::{Write, Read};
use crate::util::ioutil::ByteBuffer;
use openssl::symm::{Cipher, Mode, Crypter};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use tokio::sync::mpsc::UnboundedSender;
use crate::network::packet_handler::{ServerBoundPacket, ClientBoundPacket, serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::net::TcpStream;

#[derive(Clone, Copy, PartialEq)]
pub enum ConnectionState {
    Handshake,
    Status,
    Login,
    Play,
    Disconnected
}

pub struct IOHandle {
    operation_buffer: ByteBuffer,
    compression_threshold: i32,
    encrypter: Option<Crypter>,
    decrypter: Option<Crypter>
}

impl IOHandle {
    pub fn new() -> Self {
        IOHandle {
            operation_buffer: ByteBuffer::new(4096),
            compression_threshold: -1,
            encrypter: None,
            decrypter: None
        }
    }

    fn write_encrypted(encrypter: Option<&mut Crypter>, source: &mut ByteBuffer, temp: &mut ByteBuffer, stream: &mut TcpStream) -> Result<()> {
        if let Some(encrypter) = encrypter {
            temp.resize(source.len());
            encrypter.update(&source[..], &mut temp[..]).unwrap();
            stream.write_all(&temp[..])
        } else {
            stream.write_all(&source[..])
        }
    }

    fn decrypt_buffer(&mut self, buffer: &mut ByteBuffer, offset: usize) {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let len = buffer.len() - offset;
            self.operation_buffer.resize(len);
            self.operation_buffer.write_bytes(&buffer[offset..]);
            decrypter.update(&self.operation_buffer[..], &mut buffer[offset..]).unwrap();
        }
    }

    pub fn enable_encryption(&mut self, shared_secret: &[u8]) {
        self.encrypter = Some(Crypter::new(Cipher::aes_128_cfb8(), Mode::Encrypt, shared_secret, Some(shared_secret)).unwrap());
        self.decrypter = Some(Crypter::new(Cipher::aes_128_cfb8(), Mode::Decrypt, shared_secret, Some(shared_secret)).unwrap());
    }

    pub fn set_compression_threshold(&mut self, compression_threshold: i32) {
        self.compression_threshold = compression_threshold;
    }

    pub async fn write_packet_data(&mut self, packet_data: &mut ByteBuffer, stream: &mut TcpStream) -> Result<()> {
        self.operation_buffer.clear();
        let result: Result<()>;
        
        if self.compression_threshold >= 0 {
            if packet_data.cursor() >= self.compression_threshold as usize {
                let data_len = packet_data.cursor();
                let mut encoder = ZlibEncoder::new(self.operation_buffer.inner_mut(), Compression::default());
                encoder.write_all(&packet_data[..]).unwrap();
                encoder.finish().unwrap();
                packet_data.clear();
                packet_data.write_varint((ByteBuffer::varint_size(data_len as i32) + self.operation_buffer.len()) as i32);
                packet_data.write_varint(data_len as i32);
                packet_data.write_bytes(&self.operation_buffer[..]);

                result = IOHandle::write_encrypted(self.encrypter.as_mut(), packet_data, &mut self.operation_buffer, stream);
            } else {
                self.operation_buffer.write_varint(packet_data.len() as i32 + 1);
                self.operation_buffer.write_u8(0); // Data length of 0 signals that this packet is uncompressed
                self.operation_buffer.write_bytes(&packet_data[..]);

                result = IOHandle::write_encrypted(self.encrypter.as_mut(), &mut self.operation_buffer, packet_data, stream);
            }
        } else {
            self.operation_buffer.write_varint(packet_data.len() as i32);
            self.operation_buffer.write_bytes(&packet_data[..]);

            result = IOHandle::write_encrypted(self.encrypter.as_mut(), &mut self.operation_buffer, packet_data, stream);
        }

        packet_data.clear();
        result
    }

    pub async fn collect_packet(&mut self, packet_buffer: &mut ByteBuffer, stream: &mut TcpStream) -> Result<()> {
        self.decrypt_buffer(&mut *packet_buffer, 0);

        // Read the packet header
        let raw_len: usize = packet_buffer.read_varint() as usize;
        let mut data_len: usize;
        let compressed: bool;
        if self.compression_threshold >= 0 {
            data_len = packet_buffer.read_varint() as usize;
            if data_len == 0 {
                data_len = raw_len;
                compressed = false;
            } else {
                compressed = true;
            }
        } else {
            data_len = raw_len;
            compressed = false;
        }

        // Large packet, gather the rest of the data
        if raw_len > packet_buffer.capacity() {
            let end = packet_buffer.len();
            packet_buffer.resize(raw_len);
            match stream.read_exact(&mut packet_buffer[end..]) {
                Ok(_) => self.decrypt_buffer(&mut *packet_buffer, end),
                Err(e) => return Err(e)
            }
        }

        // Decompress the packet if needed
        if compressed {
            self.operation_buffer.reset_cursor();
            self.operation_buffer.write_bytes(&packet_buffer[packet_buffer.cursor()..]);
            packet_buffer.resize(data_len);
            packet_buffer.reset_cursor();
            let mut decoder = ZlibDecoder::new(&self.operation_buffer[..]);
            match decoder.read(&mut packet_buffer[..]) {
                Ok(read) => if read != data_len {
                    // TODO: Handle properly
                    println!("Decompression error; connection.rs");
                },
                Err(e) => return Err(e)
            };
        }

        Ok(())
    }
}

pub struct WriteHandle {
    stream: TcpStream,
    packet_buffer: ByteBuffer,
    io_handle: Arc<Mutex<IOHandle>>
}

impl WriteHandle {
    pub fn new(stream: TcpStream, io_handle: Arc<Mutex<IOHandle>>) -> Self {
        WriteHandle {
            stream,
            packet_buffer: ByteBuffer::new(4096),
            io_handle
        }
    }

    pub async fn send_packet(&mut self, packet: ClientBoundPacket) {
        serialize(packet, &mut self.packet_buffer);
        // This clears the packet buffer when done
        if let Err(e) = self.io_handle.lock().await.write_packet_data(&mut self.packet_buffer, &mut self.stream).await {
            println!("Failed to send packet: {}", e);
        }
    }
}

pub struct AsyncClientConnection {
    pub stream: TcpStream,
    pub packet_buffer: ByteBuffer,
    io_handle: Arc<Mutex<IOHandle>>,
    pub connection_state: ConnectionState,
    sync_packet_sender: UnboundedSender<ServerBoundPacket>
}

impl AsyncClientConnection {
    pub fn new(stream: TcpStream, sync_packet_sender: UnboundedSender<ServerBoundPacket>) -> Self {
        AsyncClientConnection {
            stream,
            packet_buffer: ByteBuffer::new(4096),
            io_handle: Arc::new(Mutex::new(IOHandle::new())),
            connection_state: ConnectionState::Handshake,
            sync_packet_sender
        }
    }

    pub fn create_write_handle(&self) -> WriteHandle {
        WriteHandle::new(self.stream.try_clone().expect("Failed to clone client connection stream."), self.io_handle.clone())
    }

    pub async fn send_packet(&mut self, packet: ClientBoundPacket) {
        serialize(packet, &mut self.packet_buffer);
        // This clears the packet buffer when done
        if let Err(e) = self.io_handle.lock().await.write_packet_data(&mut self.packet_buffer, &mut self.stream).await {
            println!("Failed to send packet: {}", e);
        }
    }

    pub fn forward_to_server(&mut self, packet: ServerBoundPacket) {
        if let Err(e) = self.sync_packet_sender.send(packet) {
            println!("Failed to forward synchronous packet to server: {}", e);
        }
    }

    pub async fn read_packet(&mut self) -> Result<()> {
        self.packet_buffer.inflate();
        self.packet_buffer.reset_cursor();

        // Read the first chunk, this is what blocks the thread
        match self.stream.read(&mut self.packet_buffer[..]) {
            Ok(read) => {
                if read == 0 {
                    self.connection_state = ConnectionState::Disconnected;
                    self.packet_buffer.clear();
                    Ok(())
                } else {
                    self.packet_buffer.resize(read);
                    self.io_handle.lock().await.collect_packet(&mut self.packet_buffer, &mut self.stream).await
                }
            },
            Err(e) => Err(e)
        }
    }
}