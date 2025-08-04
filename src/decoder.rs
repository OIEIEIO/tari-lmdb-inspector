// File: src/decoder.rs
// Version: v1.2.1

use std::io::{Cursor, Read};
use crate::model::BlockHeaderLite;

pub fn decode_block_header(bytes: &[u8]) -> Result<BlockHeaderLite, std::io::Error> {
    let mut rdr = Cursor::new(bytes);

    let mut buf8 = [0u8; 8];
    let mut buf2 = [0u8; 2];
    let mut buf1 = [0u8; 1];
    let mut prev_hash = [0u8; 32];

    rdr.read_exact(&mut buf8)?;
    let height = u64::from_le_bytes(buf8);

    rdr.read_exact(&mut buf2)?;
    let version = u16::from_le_bytes(buf2);

    rdr.read_exact(&mut buf8)?;
    let timestamp = u64::from_le_bytes(buf8);

    rdr.read_exact(&mut buf8)?;
    let nonce = u64::from_le_bytes(buf8);

    rdr.read_exact(&mut prev_hash)?;

    rdr.read_exact(&mut buf1)?;
    let pow_algo = buf1[0];

    rdr.read_exact(&mut buf8)?;
    let confirmations = u64::from_le_bytes(buf8);

    Ok(BlockHeaderLite {
        height, // Now used in the struct
        version,
        timestamp,
        nonce,
        previous_hash: hex::encode(prev_hash),
        pow_algo,
        confirmations,
    })
}