use crate::network::{packet_handler::*, PacketBuffer};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use futures_lite::*;
use log::*;
use openssl::{
    error::ErrorStack,
    symm::{Cipher, Crypter, Mode},
};
use smol::{
    channel::{self, Sender},
    lock::Mutex,
    net::TcpStream,
};
use std::{
    io::{Error as IoError, ErrorKind as IoErrorKind, Read, Result, Write},
    net::Shutdown,
    sync::{mpsc::Sender as StdSender, Arc},
};

/// All possible states of a client's connection to the server.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// The handshake state of the connection in which the client selects the next state to enter:
    /// either the `Status` state or `Login` state.
    Handshake,
    /// The client is requesting a server status ping.
    Status,
    /// The client is logging into the server.
    Login,
    /// The client has successfully logged into the server and is playing the game.
    Play,
    /// The client has disconnected.
    Disconnected,
}

/// Assists in pre-processing connection data, such as handling compression and encryption. If the
/// compression threshold is greater than zero, then Zlib compression is applied to packets whose
/// body exceeds the threshold size. If encryption is enabled, then AES-CFB8 encryption is used.
pub struct IOHandle {
    operation_buffer: PacketBuffer,
    compression_threshold: i32,
    encrypter: Option<Crypter>,
    decrypter: Option<Crypter>,
}

impl IOHandle {
    /// Creates a new I/O handle with an internal buffer of 4096 bytes.
    pub fn new() -> Self {
        IOHandle {
            operation_buffer: PacketBuffer::new(4096),
            compression_threshold: -1,
            encrypter: None,
            decrypter: None,
        }
    }

    /// Encrypts the given source bytes if encryption is enabled and writes them to the stream
    /// using the temporary buffer for the encryption.
    async fn write_encrypted(
        encrypter: Option<&mut Crypter>,
        source: &mut PacketBuffer,
        temp: &mut PacketBuffer,
        stream: &mut TcpStream,
    ) -> Result<()>
    {
        if let Some(encrypter) = encrypter {
            temp.resize(source.len());
            encrypter.update(&source[..], &mut temp[..])?;
            stream.write_all(&temp[..]).await
        } else {
            stream.write_all(&source[..]).await
        }
    }

    /// Decrypts the given buffer after the given offset if encryption is enabled, writing the
    /// decrypted bytes back to the buffer.
    fn decrypt_buffer(&mut self, buffer: &mut PacketBuffer, offset: usize) {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let len = buffer.len() - offset;
            self.operation_buffer.reset_cursor();
            self.operation_buffer.resize(len);
            self.operation_buffer.write_bytes(&buffer[offset ..]);

            if let Err(e) = decrypter.update(&self.operation_buffer[..], &mut buffer[offset ..]) {
                error!("Failed to decrypt packet data: {}", e);
            }
        }
    }

    /// Enables encryption with the given shared secred, initializing the internal encrypter and decrypter.
    pub fn enable_encryption(
        &mut self,
        shared_secret: &[u8],
    ) -> std::result::Result<(), ErrorStack>
    {
        self.encrypter = Some(Crypter::new(
            Cipher::aes_128_cfb8(),
            Mode::Encrypt,
            shared_secret,
            Some(shared_secret),
        )?);
        self.decrypter = Some(Crypter::new(
            Cipher::aes_128_cfb8(),
            Mode::Decrypt,
            shared_secret,
            Some(shared_secret),
        )?);
        Ok(())
    }

    /// Sets the compression threshold to the given value. Any negative value will disable compression.
    pub fn set_compression_threshold(&mut self, compression_threshold: i32) {
        self.compression_threshold = compression_threshold;
    }

    /// Writes the raw packet data bytes to the given stream, applying compression and encryption if needed.
    pub async fn write_packet_data(
        &mut self,
        packet_data: &mut PacketBuffer,
        stream: &mut TcpStream,
    ) -> Result<()>
    {
        // Prepare the operation buffer
        self.operation_buffer.clear();
        let result: Result<()>;

        // We need to check to see if the packet should be compressed
        if self.compression_threshold >= 0 {
            // We're past the compression threshold so perform the compression
            if packet_data.cursor() >= self.compression_threshold as usize {
                let data_len = packet_data.cursor();

                // Compress the packet data and write to the operation buffer
                let mut encoder =
                    ZlibEncoder::new(self.operation_buffer.inner_mut(), Compression::default());
                encoder.write_all(&packet_data[..])?;
                encoder.finish()?;

                // Use the packet data buffer to write the final packet
                packet_data.clear();

                // Raw length
                packet_data.write_varint(
                    (PacketBuffer::varint_size(data_len as i32) + self.operation_buffer.len())
                        as i32,
                );
                // Data length
                packet_data.write_varint(data_len as i32);
                packet_data.write_bytes(&self.operation_buffer[..]);

                result = IOHandle::write_encrypted(
                    self.encrypter.as_mut(),
                    packet_data,
                    &mut self.operation_buffer,
                    stream,
                )
                .await;
            }
            // The packet length is not past the threshold so no need to compress, however the header is still modified
            else {
                // Raw length
                self.operation_buffer
                    .write_varint(packet_data.len() as i32 + 1);
                // Data length of 0 signals that this packet is uncompressed
                self.operation_buffer.write_u8(0);
                self.operation_buffer.write_bytes(&packet_data[..]);

                result = IOHandle::write_encrypted(
                    self.encrypter.as_mut(),
                    &mut self.operation_buffer,
                    packet_data,
                    stream,
                )
                .await;
            }
        }
        // The packet does not need to be compressed, so just record the length and write the raw bytes
        else {
            self.operation_buffer.write_varint(packet_data.len() as i32);
            self.operation_buffer.write_bytes(&packet_data[..]);

            result = IOHandle::write_encrypted(
                self.encrypter.as_mut(),
                &mut self.operation_buffer,
                packet_data,
                stream,
            )
            .await;
        }

        result
    }

    /// Reads the packet header, collects the remaining bytes and decrypts and decompresses the packet,
    /// returning the final length of the fully processed packet. The curser of the given packet buffer
    /// will start at the packet ID.
    pub async fn collect_packet(
        &mut self,
        packet_buffer: &mut PacketBuffer,
        stream: &mut TcpStream,
        decrypt: bool,
    ) -> Result<usize>
    {
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
            match stream.read_exact(&mut packet_buffer[end ..]).await {
                Ok(_) => self.decrypt_buffer(&mut *packet_buffer, end),
                Err(e) => return Err(e),
            }
        }

        // Decompress the packet if needed
        if compressed {
            self.operation_buffer.clear();
            // Write all bytes including any potential bytes that are part of another packet
            self.operation_buffer
                .write_bytes(&packet_buffer[packet_buffer.cursor() ..]);

            // Only decompress to the end of this packet
            let compressed_end = raw_len - packet_buffer.cursor();
            let mut decoder = ZlibDecoder::new(&self.operation_buffer[.. compressed_end]);

            // Prepare the packet buffer for decompression
            packet_buffer.resize(data_len);
            packet_buffer.reset_cursor();

            match decoder.read(&mut packet_buffer[..]) {
                Ok(read) =>
                    if read != data_len {
                        return Err(IoError::new(
                            IoErrorKind::InvalidData,
                            "Failed to decompress packet",
                        ));
                    },
                Err(e) => return Err(e),
            };

            // Copy any bytes at the end of the buffer that were not part of this packet
            if self.operation_buffer.len() > compressed_end {
                packet_buffer.set_cursor(packet_buffer.len());
                packet_buffer.write_bytes(&self.operation_buffer[compressed_end ..]);
                packet_buffer.reset_cursor();
            }
        }

        Ok(data_len)
    }
}

/// A handle for asynchronously writing packets to a client connection. While the time at which the packets
/// will be processed cannot be guaranteed, packets will always be sent in the order that they are passed
/// to this handle.
pub struct AsyncWriteHandle(Sender<WrappedClientBoundPacket>);

impl AsyncWriteHandle {
    /// Attempts to send the given wrapped packet, logging an error if the operation fails.
    async fn try_send(&self, packet: WrappedClientBoundPacket) {
        if let Err(e) = self.0.send(packet).await {
            error!("Failed to forward client-bound packet to serializer: {}", e);
        }
    }

    /// Sends a packet to the client.
    pub async fn send_packet(&self, packet: ClientBoundPacket) {
        self.try_send(WrappedClientBoundPacket::Packet(packet))
            .await;
    }

    /// Sends the given raw bytes to the client.
    pub async fn send_buffer(&self, buffer: PacketBuffer) {
        self.try_send(WrappedClientBoundPacket::Buffer(buffer))
            .await;
    }

    /// Forcefully closes the connection.
    pub async fn shutdown_connection(&self) {
        self.try_send(WrappedClientBoundPacket::Disconnect).await;
    }
}

/// Manages a connection to a client. The name is a bit misleading, as this struct and its methods
/// are not asynchronous, rather this struct should used in an asynchronous context, that is not
/// on the main server thread.
pub struct AsyncClientConnection {
    /// The client ID.
    pub id: usize,
    /// The raw TCP stream the client is connected with.
    pub stream: TcpStream,
    /// The packet buffer used when reading packet bytes.
    pub read_buffer: PacketBuffer,
    /// The packet buffer used when writing packet data before sending it.
    write_buffer: PacketBuffer,
    /// The state of the connection.
    pub connection_state: ConnectionState,
    /// A handle to the packet pre-processor.
    io_handle: Arc<Mutex<IOHandle>>,
    /// A channel to forward packets to the server thread.
    sync_packet_sender: StdSender<WrappedServerBoundPacket>,
}

impl AsyncClientConnection {
    /// Creates a new connection wrapper around the given stream.
    pub fn new(
        id: usize,
        stream: TcpStream,
        sync_packet_sender: StdSender<WrappedServerBoundPacket>,
    ) -> Self
    {
        AsyncClientConnection {
            id,
            stream,
            read_buffer: PacketBuffer::new(4096),
            write_buffer: PacketBuffer::new(4096),
            io_handle: Arc::new(Mutex::new(IOHandle::new())),
            connection_state: ConnectionState::Handshake,
            sync_packet_sender,
        }
    }

    /// Creates a handle to write packets to this connection asynchronously and spawns a thread to drive the
    /// returned handle.
    pub fn create_write_handle(&self) -> AsyncWriteHandle {
        // Setup variables to be captured
        let mut stream = self.stream.clone();
        let io_handle = self.io_handle.clone();
        let (packet_sender, packet_receiver) = channel::unbounded::<WrappedClientBoundPacket>();

        // Spawn a thread to drive the returned handle
        smol::spawn(async move {
            let mut packet_buffer = PacketBuffer::new(4096);

            while let Ok(wrapped_packet) = packet_receiver.recv().await {
                match wrapped_packet {
                    WrappedClientBoundPacket::Packet(packet) => {
                        packet_buffer.clear();
                        serialize(&packet, &mut packet_buffer);

                        if let Err(e) = io_handle
                            .lock()
                            .await
                            .write_packet_data(&mut packet_buffer, &mut stream)
                            .await
                        {
                            error!("Failed to send packet: {}", e);
                        }
                    }

                    WrappedClientBoundPacket::Buffer(buffer) => {
                        if let Err(e) = stream.write_all(&buffer[..]).await {
                            error!("Failed to send buffer: {}", e);
                        }
                    }

                    WrappedClientBoundPacket::Disconnect => {
                        if let Err(e) = stream.shutdown(Shutdown::Both) {
                            error!("Failed to disconnect client: {}", e);
                        }

                        return;
                    }
                }
            }
        })
        .detach();

        AsyncWriteHandle(packet_sender)
    }

    /// Sends the given packet to the client.
    pub async fn send_packet(&mut self, packet: &ClientBoundPacket) {
        self.write_buffer.clear();
        serialize(packet, &mut self.write_buffer);

        if let Err(e) = self
            .io_handle
            .lock()
            .await
            .write_packet_data(&mut self.write_buffer, &mut self.stream)
            .await
        {
            error!("Failed to send packet: {}", e);
        }
    }

    /// Forwards the given packet to the server thread for handling.
    pub fn forward_to_server(&mut self, packet: ServerBoundPacket) {
        if let Err(e) = self
            .sync_packet_sender
            .send(WrappedServerBoundPacket::new(self.id, packet))
        {
            error!("Failed to forward synchronous packet to server: {}", e);
        }
    }

    /// Attempts to initialize encryption with the given secret key.
    pub async fn initiate_encryption(
        &mut self,
        shared_secret: &[u8],
    ) -> std::result::Result<(), ErrorStack>
    {
        self.io_handle.lock().await.enable_encryption(shared_secret)
    }

    /// Reads packet data from the underlying stream, blocking the current thread. After the initial read,
    /// the rest of the packet will be collected and read, with the number of bytes in the packet returned.
    pub async fn read_packet(&mut self) -> Result<usize> {
        // More than one packet was read at once, collect the remaining packet and handle it
        if self.read_buffer.remaining() > 0 {
            // Move the remaining bytes to the beginning of the buffer
            self.read_buffer.shift_remaining();

            // Don't decrypt the remaining bytes since that was already handled
            return self
                .io_handle
                .lock()
                .await
                .collect_packet(&mut self.read_buffer, &mut self.stream, false)
                .await;
        }
        // Prepare for the next packet
        else {
            self.read_buffer.reset_cursor();
        }

        // Inflate the buffer so we can read to its capacity
        self.read_buffer.inflate();

        // Read the first chunk, this is what blocks the thread
        let read = self.stream.read(&mut self.read_buffer[..]).await?;

        // A read of zero bytes means the stream has closed
        if read == 0 {
            self.connection_state = ConnectionState::Disconnected;
            self.read_buffer.clear();
        }
        // A packet was received
        else {
            // Adjust the buffer length to be that of the bytes read
            self.read_buffer.resize(read);

            // The legacy ping packet has no length prefix, so only collect the packet if it's not legacy
            if !(self.connection_state == ConnectionState::Handshake
                && self.read_buffer.peek() as i32 == LEGACY_PING_PACKET_ID)
            {
                return self
                    .io_handle
                    .lock()
                    .await
                    .collect_packet(&mut self.read_buffer, &mut self.stream, true)
                    .await;
            }
        }

        Ok(read)
    }

    /// Closes the underlying stream without sending a packet to the client beforehand.
    pub fn shutdown(&self) {
        if let Err(e) = self.stream.shutdown(Shutdown::Both) {
            error!("Failed to shutdown async client connection: {}", e);
        }
    }
}
