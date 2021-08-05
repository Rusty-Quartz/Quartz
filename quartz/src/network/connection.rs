use crate::network::*;

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use log::*;
use openssl::{
    error::ErrorStack,
    symm::{Cipher, Crypter, Mode},
};
use parking_lot::Mutex;
use quartz_net::{
    packets::{ClientBoundPacket, ServerBoundPacket},
    ConnectionState,
    PacketBuffer,
    PacketSerdeError,
    LEGACY_PING_PACKET_ID,
};
use std::{
    future::Future,
    io::{Error as IoError, ErrorKind as IoErrorKind, Read, Result, Write},
    result::Result as StdResult,
    sync::{mpsc::Sender as StdSender, Arc},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::mpsc::{self, UnboundedSender},
};

/// Assists in pre-processing connection data, such as handling compression and encryption. If the
/// compression threshold is greater than zero, then Zlib compression is applied to packets whose
/// body exceeds the threshold size. If encryption is enabled, then AES-CFB8 encryption is used.
pub struct IoHandle {
    compression_threshold: i32,
    encrypter: Option<Crypter>,
    decrypter: Option<Crypter>,
}

impl IoHandle {
    /// Creates a new I/O handle with an internal buffer of 4096 bytes.
    pub fn new() -> Self {
        IoHandle {
            compression_threshold: -1,
            encrypter: None,
            decrypter: None,
        }
    }

    /// Encrypts the given source bytes if encryption is enabled and writes them to the stream
    /// using the temporary buffer for the encryption.
    fn write_encrypted<'a>(
        encrypter: Option<&mut Crypter>,
        source: &'a [u8],
        temp: &'a mut PacketBuffer,
        stream: &'a mut OwnedWriteHalf,
    ) -> Result<impl Future<Output = Result<()>> + 'a> {
        let bytes = if let Some(encrypter) = encrypter {
            temp.resize(source.len());
            encrypter.update(&source[..], &mut temp[..])?;
            &temp[..]
        } else {
            &source[..]
        };

        Ok(async move { stream.write_all(bytes).await })
    }

    /// Decrypts the given buffer after the given offset if encryption is enabled, writing the
    /// decrypted bytes back to the buffer.
    fn decrypt_buffer(
        &mut self,
        buffer: &mut PacketBuffer,
        aux_buffer: &mut PacketBuffer,
        offset: usize,
    ) {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let len = buffer.len() - offset;
            aux_buffer.reset_cursor();
            aux_buffer.resize(len);
            aux_buffer.write_bytes(&buffer[offset ..]);

            if let Err(e) = decrypter.update(&aux_buffer[..], &mut buffer[offset ..]) {
                error!("Failed to decrypt packet data: {}", e);
            }
        }
    }

    /// Enables encryption with the given shared secred, initializing the internal encrypter and decrypter.
    pub fn enable_encryption(
        &mut self,
        shared_secret: &[u8],
    ) -> std::result::Result<(), ErrorStack> {
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
    fn write_packet_data<'a>(
        &mut self,
        packet_data: &'a mut PacketBuffer,
        aux_buffer: &'a mut PacketBuffer,
        stream: &'a mut OwnedWriteHalf,
    ) -> Result<impl Future<Output = Result<()>> + 'a> {
        // Prepare the operation buffer
        aux_buffer.clear();

        // We need to check to see if the packet should be compressed
        if self.compression_threshold >= 0 {
            // We're past the compression threshold so perform the compression
            if packet_data.cursor() >= self.compression_threshold as usize {
                let data_len = packet_data.cursor();

                // Compress the packet data and write to the operation buffer
                // Safety: we don't interact with the cursor of `operation_buffer` after this,
                // and the cursor is set to zero anyway, which is always valid
                let mut encoder =
                    ZlibEncoder::new(unsafe { aux_buffer.inner_mut() }, Compression::default());
                encoder.write_all(&packet_data[..])?;
                encoder.finish()?;

                // Use the packet data buffer to write the final packet
                packet_data.clear();

                // Raw length
                packet_data.write_varying(
                    &((PacketBuffer::varint_size(data_len as i32) + aux_buffer.len()) as i32),
                );
                // Data length
                packet_data.write_varying(&(data_len as i32));
                packet_data.write_bytes(&aux_buffer[..]);

                IoHandle::write_encrypted(
                    self.encrypter.as_mut(),
                    &packet_data[..],
                    aux_buffer,
                    stream,
                )
            }
            // The packet length is not past the threshold so no need to compress, however the header is still modified
            else {
                // Raw length
                aux_buffer.write_varying(&(packet_data.len() as i32 + 1));
                // Data length of 0 signals that this packet is uncompressed
                aux_buffer.write_one(0);
                aux_buffer.write_bytes(&packet_data[..]);

                IoHandle::write_encrypted(
                    self.encrypter.as_mut(),
                    &aux_buffer[..],
                    packet_data,
                    stream,
                )
            }
        }
        // The packet does not need to be compressed, so just record the length and write the raw bytes
        else {
            aux_buffer.write_varying(&(packet_data.len() as i32));
            aux_buffer.write_bytes(&packet_data[..]);

            IoHandle::write_encrypted(
                self.encrypter.as_mut(),
                &aux_buffer[..],
                packet_data,
                stream,
            )
        }
    }

    /// Reads the packet header, collects the remaining bytes and decrypts and decompresses the packet,
    /// returning the final length of the fully processed packet. The curser of the given packet buffer
    /// will start at the packet ID.
    fn collect_packet<'a>(
        &mut self,
        packet_buffer: &'a mut PacketBuffer,
        aux_buffer: &'a mut PacketBuffer,
        stream: &'a mut OwnedReadHalf,
        decrypt: bool,
    ) -> StdResult<
        impl Future<Output = StdResult<CollectedPacket<'a>, PacketSerdeError>>,
        PacketSerdeError,
    > {
        if decrypt {
            self.decrypt_buffer(&mut *packet_buffer, aux_buffer, 0);
        }

        // Read the packet header

        // Length of the packet in its raw, unaltered form
        let raw_len = packet_buffer.read_varying::<i32>()? as usize;
        // Length of the uncompressed packet data excluding the raw length header
        let mut data_len: usize;
        let compressed: bool;

        // Compression is active
        if self.compression_threshold >= 0 {
            // Read the length of the uncompressed packet data
            data_len = packet_buffer.read_varying::<i32>()? as usize;

            // If that length is zero, the packet was not compressed
            if data_len == 0 {
                data_len = raw_len - PacketBuffer::varint_size(raw_len as i32);
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

        Ok(async move {
            if raw_len <= packet_buffer.len() {
                Ok(CollectedPacket {
                    packet_buffer,
                    aux_buffer,
                    raw_len,
                    data_len,
                    encrypted_offset: None,
                    compressed,
                })
            }
            // Large packet, gather the rest of the data
            else {
                let end = packet_buffer.len();
                packet_buffer.resize(PacketBuffer::varint_size(raw_len as i32) + raw_len);
                match stream.read_exact(&mut packet_buffer[end ..]).await {
                    Ok(_) => Ok(CollectedPacket {
                        packet_buffer,
                        aux_buffer,
                        raw_len,
                        data_len,
                        encrypted_offset: Some(end),
                        compressed,
                    }),
                    Err(e) => Err(PacketSerdeError::Network(e)),
                }
            }
        })
    }

    fn finalize_packet(
        this: &Mutex<Self>,
        packet: CollectedPacket<'_>,
    ) -> StdResult<usize, PacketSerdeError> {
        let CollectedPacket {
            packet_buffer,
            aux_buffer,
            raw_len,
            data_len,
            encrypted_offset,
            compressed,
        } = packet;

        if let Some(offset) = encrypted_offset {
            this.lock()
                .decrypt_buffer(&mut *packet_buffer, aux_buffer, offset);
        }

        // Decompress the packet if needed
        if compressed {
            aux_buffer.clear();
            // Write all bytes including any potential bytes that are part of another packet
            aux_buffer.write_bytes(&packet_buffer[packet_buffer.cursor() ..]);

            // Only decompress to the end of this packet
            // Cursor = vsize(raw_len) + vsize(data_len)
            // compressed_end = raw_len - vsize(raw_len)
            let compressed_end = (raw_len + PacketBuffer::varint_size(data_len as i32)) - packet_buffer.cursor();
            let mut decoder = ZlibDecoder::new(&aux_buffer[.. compressed_end]);

            // Prepare the packet buffer for decompression
            packet_buffer.resize(data_len);
            packet_buffer.reset_cursor();

            match decoder.read(&mut packet_buffer[..]) {
                Ok(read) =>
                    if read != data_len {
                        return Err(PacketSerdeError::Network(
                            IoError::new(IoErrorKind::InvalidData, "Failed to decompress packet")
                                .into(),
                        ));
                    },
                Err(e) => return Err(PacketSerdeError::Network(e.into())),
            };

            // Copy any bytes at the end of the buffer that were not part of this packet
            if aux_buffer.len() > compressed_end {
                packet_buffer.set_cursor(packet_buffer.len());
                packet_buffer.write_bytes(&aux_buffer[compressed_end ..]);
                packet_buffer.reset_cursor();
            }
        }

        Ok(data_len)
    }
}

struct CollectedPacket<'a> {
    packet_buffer: &'a mut PacketBuffer,
    aux_buffer: &'a mut PacketBuffer,
    raw_len: usize,
    data_len: usize,
    encrypted_offset: Option<usize>,
    compressed: bool,
}

/// A handle for asynchronously writing packets to a client connection. While the time at which the packets
/// will be processed cannot be guaranteed, packets will always be sent in the order that they are passed
/// to this handle.
// TODO: consider bounding the channel
#[derive(Clone, Debug)]
pub struct AsyncWriteHandle(UnboundedSender<WrappedClientBoundPacket>);

impl AsyncWriteHandle {
    // These functions are async so that if we bound the channel it's not a breaking change

    /// Attempts to send the given wrapped packet, logging an error if the operation fails.
    async fn try_send(&self, packet: WrappedClientBoundPacket) {
        if let Err(e) = self.0.send(packet) {
            error!("Failed to forward client-bound packet to serializer: {}", e);
        }
    }

    /// Sends a packet to the client.
    pub async fn send_packet(&self, packet: impl Into<WrappedClientBoundPacket>) {
        self.try_send(packet.into()).await;
    }

    pub async fn send_all<I>(&self, packets: I)
    where
        I: IntoIterator,
        I::Item: Into<WrappedClientBoundPacket>,
    {
        let packets = packets
            .into_iter()
            .map(|packet| packet.into())
            .collect::<Box<[_]>>();
        self.try_send(WrappedClientBoundPacket::Multiple(packets))
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
    read_handle: OwnedReadHalf,
    pub write_handle: AsyncWriteHandle,
    /// The packet buffer used when reading packet bytes.
    pub read_buffer: PacketBuffer,
    aux_buffer: PacketBuffer,
    /// The state of the connection.
    pub connection_state: ConnectionState,
    /// A handle to the packet pre-processor.
    io_handle: Arc<Mutex<IoHandle>>,
    /// A channel to forward packets to the server thread.
    sync_packet_sender: StdSender<WrappedServerBoundPacket>,
}

impl AsyncClientConnection {
    /// Creates a new connection wrapper around the given stream.
    pub fn new(
        id: usize,
        stream: TcpStream,
        sync_packet_sender: StdSender<WrappedServerBoundPacket>,
    ) -> (Self, impl Future<Output = ()>) {
        let (read_handle, write_handle) = stream.into_split();
        let io_handle = Arc::new(Mutex::new(IoHandle::new()));
        let (write_handle, driver) = Self::create_write_handle(write_handle, io_handle.clone());

        let conn = AsyncClientConnection {
            id,
            read_handle,
            write_handle,
            read_buffer: PacketBuffer::new(4096),
            aux_buffer: PacketBuffer::new(4096),
            io_handle,
            connection_state: ConnectionState::Handshake,
            sync_packet_sender,
        };

        (conn, driver)
    }

    fn create_write_handle(
        mut write_handle: OwnedWriteHalf,
        io_handle: Arc<Mutex<IoHandle>>,
    ) -> (AsyncWriteHandle, impl Future<Output = ()>) {
        let (packet_sender, mut packet_receiver) =
            mpsc::unbounded_channel::<WrappedClientBoundPacket>();

        // Create a future to drive the handle
        let driver = async move {
            let mut packet_buffer = PacketBuffer::new(4096);
            let mut aux_buffer = PacketBuffer::new(4096);

            while let Some(wrapped_packet) = packet_receiver.recv().await {
                if Self::write_wrapped_packet(
                    wrapped_packet,
                    &mut packet_buffer,
                    &mut aux_buffer,
                    &mut write_handle,
                    &*io_handle,
                )
                .await
                {
                    if let Err(e) = write_handle.shutdown().await {
                        warn!("Failed to disconnect client: {}", e);
                    }
                    return;
                }

                let _ = write_handle.flush().await;
            }
        };

        (AsyncWriteHandle(packet_sender), driver)
    }

    async fn write_wrapped_packet(
        wrapped_packet: WrappedClientBoundPacket,
        buffer: &mut PacketBuffer,
        aux_buffer: &mut PacketBuffer,
        write_handle: &mut OwnedWriteHalf,
        io_handle: &Mutex<IoHandle>,
    ) -> bool {
        async fn write_buffer(
            buffer: &mut PacketBuffer,
            aux_buffer: &mut PacketBuffer,
            write_handle: &mut OwnedWriteHalf,
            io_handle: &Mutex<IoHandle>,
        ) {
            let write_fut = io_handle
                .lock()
                .write_packet_data(buffer, aux_buffer, write_handle);
            if let Ok(fut) = write_fut {
                let _ = fut.await;
            }
        }

        async fn write_raw_buffer(buffer: &PacketBuffer, write_handle: &mut OwnedWriteHalf) {
            let _ = write_handle.write_all(&buffer[..]).await;
        }

        match wrapped_packet {
            WrappedClientBoundPacket::Singleton(packet) => {
                buffer.clear();
                buffer.write(&packet);
                write_buffer(buffer, aux_buffer, write_handle, io_handle).await;
            }

            WrappedClientBoundPacket::Multiple(packets) => {
                let mut disconnect_when_done = false;
                let mut flush = false;

                for packet in packets.iter() {
                    buffer.clear();

                    match packet {
                        WrappedClientBoundPacket::Singleton(packet) => buffer.write(packet),
                        WrappedClientBoundPacket::Custom(packet) => buffer.write(&**packet),
                        WrappedClientBoundPacket::Buffer(buffer) => {
                            write_raw_buffer(buffer, write_handle).await;
                            continue;
                        }
                        WrappedClientBoundPacket::Flush => flush = true,
                        WrappedClientBoundPacket::Disconnect => disconnect_when_done = true,
                        WrappedClientBoundPacket::EnableCompression { .. } => warn!(
                            "Attempted to send compression-enabling packet in multi-packet \
                             payload. This packet will be dropped."
                        ),
                        WrappedClientBoundPacket::Multiple(..) => {
                            warn!(
                                "Attempted to write nested \
                                 WrappedClientBoundPacket::Multiple(..), these packets will be \
                                 dropped"
                            )
                        }
                    }

                    write_buffer(buffer, aux_buffer, write_handle, io_handle).await;
                }

                if flush {
                    if let Err(e) = write_handle.flush().await {
                        error!("Failed to flush connection socket: {}", e);
                    }
                }

                return disconnect_when_done;
            }

            WrappedClientBoundPacket::Buffer(buffer) =>
                write_raw_buffer(&buffer, write_handle).await,

            WrappedClientBoundPacket::Custom(packet) => {
                buffer.clear();
                buffer.write(&*packet);
                write_buffer(buffer, aux_buffer, write_handle, io_handle).await;
            }

            WrappedClientBoundPacket::EnableCompression { threshold } => {
                buffer.clear();
                buffer.write(&ClientBoundPacket::SetCompression { threshold });

                let write_fut = {
                    let mut guard = io_handle.lock();
                    let write_fut = guard.write_packet_data(buffer, aux_buffer, write_handle);
                    guard.set_compression_threshold(threshold);
                    write_fut
                };

                if let Ok(fut) = write_fut {
                    let _ = fut.await;
                }
            }

            WrappedClientBoundPacket::Flush => {
                if let Err(e) = write_handle.flush().await {
                    error!("Failed to flush connection socket: {}", e);
                }
            },

            WrappedClientBoundPacket::Disconnect => return true,
        }

        false
    }

    /// Forwards the given packet to the server thread for handling.
    pub fn forward_to_server(&mut self, packet: ServerBoundPacket) {
        if let Err(e) = self
            .sync_packet_sender
            .send(WrappedServerBoundPacket::external(self.id, packet))
        {
            error!("Failed to forward synchronous packet to server: {}", e);
        }
    }

    /// Forwards an internal packet to the server thread for handling.
    pub fn forward_internal_to_server(&mut self, packet: InternalPacket) {
        if let Err(e) = self
            .sync_packet_sender
            .send(WrappedServerBoundPacket::internal(self.id, packet))
        {
            error!(
                "Failed to forward synchronous internal packet to server: {}",
                e
            );
        }
    }

    /// Attempts to initialize encryption with the given secret key.
    pub fn initiate_encryption(&self, shared_secret: &[u8]) -> StdResult<(), PacketSerdeError> {
        self.io_handle
            .lock()
            .enable_encryption(shared_secret)
            .map_err(Into::into)
    }

    pub fn set_compression_threshold(&self, compression_threshold: i32) {
        self.io_handle
            .lock()
            .set_compression_threshold(compression_threshold);
    }

    /// Reads packet data from the underlying stream, blocking the current thread. After the initial read,
    /// the rest of the packet will be collected and read, with the number of bytes in the packet returned.
    pub async fn read_packet(&mut self) -> StdResult<usize, PacketSerdeError> {
        // More than one packet was read at once, collect the remaining packet and handle it
        if self.read_buffer.remaining() > 0 {
            // Move the remaining bytes to the beginning of the buffer
            self.read_buffer.shift_remaining();

            // Don't decrypt the remaining bytes since that was already handled
            let collect_fut = self.io_handle.lock().collect_packet(
                &mut self.read_buffer,
                &mut self.aux_buffer,
                &mut self.read_handle,
                false,
            )?;
            let collected = collect_fut.await?;
            return IoHandle::finalize_packet(&*self.io_handle, collected);
        }
        // Prepare for the next packet
        else {
            self.read_buffer.reset_cursor();
        }

        // Inflate the buffer so we can read to its capacity
        self.read_buffer.inflate();

        // Read the first chunk, this is what blocks the thread
        let read = self
            .read_handle
            .read(&mut self.read_buffer[..])
            .await
            .map_err(|error| PacketSerdeError::Network(error))?;

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
                && self.read_buffer.peek_one().unwrap_or(0) as i32 == LEGACY_PING_PACKET_ID)
            {
                let collect_fut = self.io_handle.lock().collect_packet(
                    &mut self.read_buffer,
                    &mut self.aux_buffer,
                    &mut self.read_handle,
                    true,
                )?;
                let collected = collect_fut.await?;
                return IoHandle::finalize_packet(&*self.io_handle, collected);
            }
        }

        // This is only reached if read == 0 or it's a legacy packet
        Ok(read)
    }
}
