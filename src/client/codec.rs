use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{cmp, fmt, io, str, usize};
use tokio_util::codec::{Decoder, Encoder};

const DEFAULT_SEEK_DELIMITERS: &[u8] = b"\r\n";
const DEFAULT_SEQUENCE_WRITER: &[u8] = b"\r\n";

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RPCDelimiterCodec {
    // Stored index of the next index to examine for the delimiter character.
    // This is used to optimize searching.
    // For example, if `decode` was called with `abc` and the delimiter is '{}', it would hold `3`,
    // because that is the next index to examine.
    // The next time `decode` is called with `abcde}`, the method will
    // only look at `de}` before returning.
    next_index: usize,

    /// The maximum length for a given chunk. If `usize::MAX`, chunks will be
    /// read until a delimiter character is reached.
    max_length: usize,

    /// Are we currently discarding the remainder of a chunk which was over
    /// the length limit?
    is_discarding: bool,

    /// The bytes that are using for search during decode
    seek_delimiters: Vec<u8>,

    /// The bytes that are using for encoding
    sequence_writer: Vec<u8>,
}

impl RPCDelimiterCodec {
    /// Returns a `RPCDelimiterCodec` for splitting up data into chunks.
    ///
    /// # Note
    ///
    /// The returned `RPCDelimiterCodec` will not have an upper bound on the length
    /// of a buffered chunk. See the documentation for [`new_with_max_length`]
    /// for information on why this could be a potential security risk.
    ///
    /// [`new_with_max_length`]: crate::codec::RPCDelimiterCodec::new_with_max_length()
    pub fn new(seek_delimiters: Vec<u8>, sequence_writer: Vec<u8>) -> RPCDelimiterCodec {
        RPCDelimiterCodec {
            next_index: 0,
            max_length: usize::MAX,
            is_discarding: false,
            seek_delimiters,
            sequence_writer,
        }
    }
}

impl Decoder for RPCDelimiterCodec {
    type Item = Bytes;
    type Error = RPCDelimiterCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, RPCDelimiterCodecError> {
        loop {
            // Determine how far into the buffer we'll search for a delimiter. If
            // there's no max_length set, we'll read to the end of the buffer.
            let read_to = cmp::min(self.max_length.saturating_add(1), buf.len());
            let seek_delimiters_length = self.seek_delimiters.len();

            let new_chunk_offset = buf[self.next_index..read_to]
                .windows(seek_delimiters_length)
                .position(|b| *b == self.seek_delimiters);

            match (self.is_discarding, new_chunk_offset) {
                (true, Some(offset)) => {
                    // If we found a new chunk, discard up to that offset and
                    // then stop discarding. On the next iteration, we'll try
                    // to read a chunk normally.
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    // Otherwise, we didn't find a new chunk, so we'll discard
                    // everything we read. On the next iteration, we'll continue
                    // discarding up to max_len bytes unless we find a new chunk.
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    // Found a chunk!
                    let new_chunk_index = offset + self.next_index;
                    self.next_index = 0;
                    let mut chunk = buf.split_to(new_chunk_index + 1);
                    chunk.truncate(chunk.len() - 1);
                    let chunk = chunk.freeze();
                    return Ok(Some(chunk));
                }
                (false, None) if buf.len() > self.max_length => {
                    // Reached the maximum length without finding a
                    // new chunk, return an error and start discarding on the
                    // next call.
                    self.is_discarding = true;
                    return Err(RPCDelimiterCodecError::MaxChunkLengthExceeded);
                }
                (false, None) => {
                    // We didn't find a chunk or reach the length limit, so the next
                    // call will resume searching at the current offset.
                    // We also need to go back the length of seek delimiters minus 1
                    // so that we can catch the delimiters that appear across chunks
                    if read_to > seek_delimiters_length {
                        self.next_index = read_to - (seek_delimiters_length - 1);
                    } else {
                        self.next_index = 0;
                    }
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, RPCDelimiterCodecError> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                // return remaining data, if any
                if buf.is_empty() {
                    None
                } else {
                    let chunk = buf.split_to(buf.len());
                    self.next_index = 0;
                    Some(chunk.freeze())
                }
            }
        })
    }
}

impl<T> Encoder<T> for RPCDelimiterCodec
where
    T: AsRef<str>,
{
    type Error = RPCDelimiterCodecError;

    fn encode(&mut self, chunk: T, buf: &mut BytesMut) -> Result<(), RPCDelimiterCodecError> {
        let chunk = chunk.as_ref();
        buf.reserve(chunk.len() + 1);
        buf.put(chunk.as_bytes());
        buf.put(self.sequence_writer.as_ref());

        Ok(())
    }
}

impl Default for RPCDelimiterCodec {
    fn default() -> Self {
        Self::new(
            DEFAULT_SEEK_DELIMITERS.to_vec(),
            DEFAULT_SEQUENCE_WRITER.to_vec(),
        )
    }
}

/// An error occurred while encoding or decoding a chunk.
#[derive(Debug)]
pub enum RPCDelimiterCodecError {
    /// The maximum chunk length was exceeded.
    MaxChunkLengthExceeded,
    /// An IO error occurred.
    Io(io::Error),
}

impl fmt::Display for RPCDelimiterCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RPCDelimiterCodecError::MaxChunkLengthExceeded => {
                write!(f, "max chunk length exceeded")
            }
            RPCDelimiterCodecError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for RPCDelimiterCodecError {
    fn from(e: io::Error) -> RPCDelimiterCodecError {
        RPCDelimiterCodecError::Io(e)
    }
}

impl std::error::Error for RPCDelimiterCodecError {}
