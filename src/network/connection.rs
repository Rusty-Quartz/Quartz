use std::io::Result;
use std::io::{Write, Read};
use crate::util::ioutil::ByteBuffer;
use openssl::symm::{Cipher, Mode, Crypter};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use futures::channel::mpsc::UnboundedSender;
use crate::network::packet_handler::*;
use std::sync::{Arc, Mutex};
use std::net::TcpStream;
use log::*;

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

    // Encrypts the given source bytes if encryption is enabled and writes them to the stream
    // using the temporary buffer for the encryption.
    fn write_encrypted(encrypter: Option<&mut Crypter>, source: &mut ByteBuffer, temp: &mut ByteBuffer, stream: &mut TcpStream) -> Result<()> {
        if let Some(encrypter) = encrypter {
            temp.resize(source.len());
            encrypter.update(&source[..], &mut temp[..]).unwrap();
            stream.write_all(&temp[..])
        } else {
            stream.write_all(&source[..])
        }
    }

    // Decrypts the given buffer after the given offser if encryption is enabled, writing the
    // decrypted bytes back to the buffer.
    fn decrypt_buffer(&mut self, buffer: &mut ByteBuffer, offset: usize) {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let len = buffer.len() - offset;
            self.operation_buffer.reset_cursor();
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

    pub fn write_packet_data(&mut self, packet_data: &mut ByteBuffer, stream: &mut TcpStream) -> Result<()> {
        // Prepare the operation buffer
        self.operation_buffer.clear();
        let result: Result<()>;
        
        // We need to check to see if the packet should be compressed
        if self.compression_threshold >= 0 {
            // We're past the compression threshold so perform the compression
            if packet_data.cursor() >= self.compression_threshold as usize {
                let data_len = packet_data.cursor();

                // Compress the packet data and write to the operation buffer
                let mut encoder = ZlibEncoder::new(self.operation_buffer.inner_mut(), Compression::default());
                encoder.write_all(&packet_data[..]).unwrap();
                encoder.finish().unwrap();

                // Use the packet data buffer to write the final packet
                packet_data.clear();

                // Raw length
                packet_data.write_varint((ByteBuffer::varint_size(data_len as i32) + self.operation_buffer.len()) as i32);
                // Data length
                packet_data.write_varint(data_len as i32);
                packet_data.write_bytes(&self.operation_buffer[..]);

                result = IOHandle::write_encrypted(self.encrypter.as_mut(), packet_data, &mut self.operation_buffer, stream);
            }
            // The packet length is not past the threshold so no need to compress, however the header is still modified
            else {
                // Raw length
                self.operation_buffer.write_varint(packet_data.len() as i32 + 1);
                // Data length of 0 signals that this packet is uncompressed
                self.operation_buffer.write_u8(0);
                self.operation_buffer.write_bytes(&packet_data[..]);

                result = IOHandle::write_encrypted(self.encrypter.as_mut(), &mut self.operation_buffer, packet_data, stream);
            }
        }
        // The packet does not need to be compressed, so just record the length and write the raw bytes
        else {
            self.operation_buffer.write_varint(packet_data.len() as i32);
            self.operation_buffer.write_bytes(&packet_data[..]);

            result = IOHandle::write_encrypted(self.encrypter.as_mut(), &mut self.operation_buffer, packet_data, stream);
        }

        result
    }

    // Reads the packet header, collects the remaining bytes and decrypts and decompresses the packet,
    // returning the final length of the fully processed packet. The curser of the given packet buffer
    // will start at the packet ID.
    pub fn collect_packet(&mut self, packet_buffer: &mut ByteBuffer, stream: &mut TcpStream, decrypt: bool) -> Result<usize> {
        if decrypt {
            self.decrypt_buffer(&mut *packet_buffer, 0);
        }

        // Read the packet header

        // Length of the packet in its raw, unaltered form
        let raw_len: usize = packet_buffer.read_varint() as usize;
        // Length of the uncompressed packet data exluding the raw length header
        let mut data_len: usize;
        let compressed: bool;

        // Compression is active
        if self.compression_threshold >= 0 {
            // Read the length of the uncompressed packet data
            data_len = packet_buffer.read_varint() as usize;

            // If that length is zero, the packet was not compressed
            if data_len == 0 {
                data_len = raw_len;
                compressed = false;
            }
            // The packet is compressed
            else {
                compressed = true;
            }
        }
        // Compression is not active
        else {
            data_len = raw_len;
            compressed = false;
        }

        // Large packet, gather the rest of the data
        if raw_len > packet_buffer.len() {
            let end = packet_buffer.len();
            packet_buffer.resize(raw_len);
            match stream.read_exact(&mut packet_buffer[end..]) {
                Ok(_) => self.decrypt_buffer(&mut *packet_buffer, end),
                Err(e) => return Err(e)
            }
        }

        // Decompress the packet if needed
        if compressed {
            self.operation_buffer.clear();
            // Write all bytes including any potential bytes that are part of another packet
            self.operation_buffer.write_bytes(&packet_buffer[packet_buffer.cursor()..]);

            // Only decompress to the end of this packet
            let compressed_end = raw_len - packet_buffer.cursor();
            let mut decoder = ZlibDecoder::new(&self.operation_buffer[..compressed_end]);

            // Prepare the packet buffer for decompression
            packet_buffer.resize(data_len);
            packet_buffer.reset_cursor();

            match decoder.read(&mut packet_buffer[..]) {
                Ok(read) => if read != data_len {
                    // TODO: Handle properly
                    error!("Failed to decompress packet");
                },
                Err(e) => return Err(e)
            };

            // Copy any bytes at the end of the buffer that were not part of this packet
            if self.operation_buffer.len() > compressed_end {
                packet_buffer.set_cursor(packet_buffer.len());
                packet_buffer.write_bytes(&self.operation_buffer[compressed_end..]);
                packet_buffer.reset_cursor();
            }
        }

        Ok(data_len)
    }
}

// A handle for writing packets to a client connection
pub struct WriteHandle {
    pub client_id: usize,
    stream: TcpStream,
    packet_buffer: ByteBuffer,
    io_handle: Arc<Mutex<IOHandle>>
}

impl WriteHandle {
    pub fn new(client_id: usize, stream: TcpStream, io_handle: Arc<Mutex<IOHandle>>) -> Self {
        WriteHandle {
            client_id,
            stream,
            packet_buffer: ByteBuffer::new(4096),
            io_handle
        }
    }

    pub fn send_packet(&mut self, packet: &ClientBoundPacket) {
        self.packet_buffer.clear();
        serialize(packet, &mut self.packet_buffer);

        if let Err(e) = self.io_handle.lock().unwrap().write_packet_data(&mut self.packet_buffer, &mut self.stream) {
            error!("Failed to send packet: {}", e);
        }
    }

    pub fn send_buffer(&mut self, buffer: &ByteBuffer) {
        if let Err(e) = self.stream.write_all(&buffer[..]) {
            error!("Failed to send buffer: {}", e);
        }
    }
}

pub struct AsyncClientConnection {
    pub id: usize,
    pub stream: TcpStream,
    pub read_buffer: ByteBuffer,
    write_buffer: ByteBuffer,
    io_handle: Arc<Mutex<IOHandle>>,
    pub connection_state: ConnectionState,
    sync_packet_sender: UnboundedSender<WrappedServerPacket>
}

impl AsyncClientConnection {
    pub fn new(id: usize, stream: TcpStream, sync_packet_sender: UnboundedSender<WrappedServerPacket>) -> Self {
        AsyncClientConnection {
            id,
            stream,
            read_buffer: ByteBuffer::new(4096),
            write_buffer: ByteBuffer::new(4096),
            io_handle: Arc::new(Mutex::new(IOHandle::new())),
            connection_state: ConnectionState::Handshake,
            sync_packet_sender
        }
    }

    pub fn create_write_handle(&self) -> WriteHandle {
        WriteHandle::new(self.id, self.stream.try_clone().expect("Failed to clone client connection stream"), self.io_handle.clone())
    }

    pub fn send_packet(&mut self, packet: &ClientBoundPacket) {
        self.write_buffer.clear();
        serialize(packet, &mut self.write_buffer);

        if let Err(e) = self.io_handle.lock().unwrap().write_packet_data(&mut self.write_buffer, &mut self.stream) {
            error!("Failed to send packet: {}", e);
        }
    }

    pub fn forward_to_server(&mut self, packet: ServerBoundPacket) {
        if let Err(e) = self.sync_packet_sender.unbounded_send(WrappedServerPacket::new(self.id, packet)) {
            error!("Failed to forward synchronous packet to server: {}", e);
        }
    }

    pub fn initiate_encryption(&mut self, shared_secret: &[u8]) {
        self.io_handle.lock().unwrap().enable_encryption(shared_secret)
    }

    pub fn read_packet(&mut self) -> Result<usize> {
        // More than one packet was read at once, collect the remaining packet and handle it
        if self.read_buffer.remaining() > 0 {
            // Move the remaining bytes to the beginning of the buffer
            self.read_buffer.zero_remaining();

            // Don't decrypt the remaining bytes since that was already handled
            return self.io_handle.lock().unwrap().collect_packet(&mut self.read_buffer, &mut self.stream, false)
        }
        // Prepare for the next packet
        else {
            self.read_buffer.reset_cursor();
        }

        // Inflate the buffer so we can read to its capacity
        self.read_buffer.inflate();

        // Read the first chunk, this is what blocks the thread
        match self.stream.read(&mut self.read_buffer[..]) {
            Ok(read) => {
                // A read of zero bytes means the stream has closed
                if read == 0 {
                    self.connection_state = ConnectionState::Disconnected;
                    self.read_buffer.clear();
                    Ok(0)
                }
                // A packet was received
                else {
                    // Adjust the buffer length to be that of the bytes read
                    self.read_buffer.resize(read);

                    // The legacy ping packet has no length prefix, so only collect the packet if it's not legacy
                    if !(self.connection_state == ConnectionState::Handshake && self.read_buffer.peek() as i32 == LEGACY_PING_PACKET_ID) {
                        return self.io_handle.lock().unwrap().collect_packet(&mut self.read_buffer, &mut self.stream, true);
                    }

                    Ok(read)
                }
            },
            Err(e) => Err(e)
        }
    }
}