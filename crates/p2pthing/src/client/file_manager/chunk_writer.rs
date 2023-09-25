use std::{
    collections::HashSet,
    fmt,
    fs::File,
    io::{self, Write},
};

use memmap::MmapMut;

pub(crate) enum Error {
    IOError(io::Error),
    InvalidChunkError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IOError(e) => fmt::Display::fmt(e, f),
            Error::InvalidChunkError => f.write_str("Tried writing a chunk which has an invalid length."),
        }
    }
}

pub(crate) struct ChunkWriter {
    inner: File,
    written_chunks: Vec<usize>,
    mmap: MmapMut,
    chunk_size: usize,
    chunk_count: usize,
}

/// This struct is similar to BufWriter as it uses buffering to optimize writing to disk,
/// however it has a few differences.
///
/// This struct stores data in a buffer before writing which has double the size of MAX_PACKET_SIZE * CHUNK_COUNT
/// this is because packets aren't arriving in order.
impl ChunkWriter {
    pub fn new(inner: File, chunk_size: usize, chunk_count: usize) -> ChunkWriter {
        let mmap = unsafe { MmapMut::map_mut(&inner).unwrap() };
        ChunkWriter { inner, written_chunks: Vec::new(), mmap, chunk_size, chunk_count }
    }

    pub fn write_chunk(&mut self, chunk_id: usize, data: &[u8]) -> Result<(), Error> {
        if self.written_chunks.contains(&chunk_id) {
            return Err(Error::InvalidChunkError);
        }

        // Write to the circular buffer
        let index_start = chunk_id * self.chunk_size;
        let index_end = index_start + data.len();
        self.mmap[index_start..index_end].copy_from_slice(&data[0..data.len()]);

        // FIXME: Don't assume that the chunk was actually written to disk until we properly flush or close the memory map

        Ok(())
    }

    /// Only write until the last block index that was pushed into the buffer
    pub fn flush(&mut self) -> Result<(), io::Error> {
        self.mmap.flush()
    }
}

impl Drop for ChunkWriter {
    fn drop(&mut self) {
        self.flush().expect("Failed flushing: ");
    }
}
