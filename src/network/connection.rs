use std::net::TcpStream;
use std::io::Read;
use std::io::Result;
use crate::util::ioutil::ByteBuffer;
use openssl::symm::{Cipher, Mode, Crypter};
use flate2::read::GzDecoder;

#[derive(Clone, Copy, PartialEq)]
pub enum State {
    Handshaking,
    Status,
    Login,
    Play,
    Disconnected
}

pub struct ClientConnection {
    pub stream: TcpStream,
    pub buffer: ByteBuffer,
    op_buffer: Vec<u8>,
    compression_threshold: i32,
    encrypter: Option<Crypter>,
    decrypter: Option<Crypter>,
    pub state: State
}

impl ClientConnection {
    pub fn new(stream: TcpStream) -> ClientConnection {
        ClientConnection {
            stream,
            buffer: ByteBuffer::new(4096),
            op_buffer: Vec::with_capacity(4096),
            compression_threshold: -1,
            encrypter: None,
            decrypter: None,
            state: State::Handshaking
        }
    }

    fn decrypt_buffer(&mut self, offset: usize) {
        if self.decrypter.is_some() {
            let len = self.buffer.end() - offset;
            self.op_buffer.reserve_exact(len);
            self.op_buffer.copy_from_slice(&self.buffer[offset..]);
            self.decrypter.as_mut().unwrap().update(&self.op_buffer[..len], &mut self.buffer[offset..]).unwrap();
        }
    }

    pub fn next_packet(&mut self) -> Result<()> {
        self.buffer.clear();

        // Read the first chunk, this is what blocks the thread
        match self.stream.read(self.buffer.as_mut()) {
            Ok(_) => self.decrypt_buffer(0),
            Err(e) => return Err(e)
        };

        // Read the packet header
        let raw_len: usize = self.buffer.read_varint() as usize;
        let mut data_len: usize;
        let compressed: bool;
        if self.compression_threshold >= 0 {
            data_len = self.buffer.read_varint() as usize;
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
        if raw_len > self.buffer.capacity() {
            let end = self.buffer.end();
            self.buffer.ensure_capacity(raw_len);
            match self.stream.read_exact(&mut self.buffer[end..]) {
                Ok(_) => self.decrypt_buffer(end),
                Err(e) => return Err(e)
            }
        }

        // Decompress the packet if needed
        if compressed {
            self.op_buffer.reserve_exact(raw_len);
            self.op_buffer.copy_from_slice(&self.buffer[self.buffer.cursor()..]);
            self.buffer.clear();
            self.buffer.ensure_capacity(data_len);
            let mut decoder = GzDecoder::new(&self.op_buffer[..]);
            match decoder.read(self.buffer.as_mut()) {
                Ok(read) => if read != data_len {
                    // TODO: Handle properly
                    println!("Decompression error; connection.rs");
                },
                Err(e) => return Err(e)
            }
        }

        Ok(())
    }
}