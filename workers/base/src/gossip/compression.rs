//! Compression related utilities.
use std::io::Read;

use eyre::{Result, WrapErr};

/// Compress data using brotli.
pub fn compress(src: &[u8]) -> Result<Vec<u8>> {
    let mut reader = brotli::CompressorReader::new(src, 4096, 11, 4096);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .wrap_err("Compression error")?;
    Ok(buffer)
}

/// Decompress data using brotli.
pub fn decompress(src: &[u8]) -> Result<Vec<u8>> {
    let mut reader = brotli::Decompressor::new(src, 4096);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .wrap_err("Decompression error")?;
    Ok(buffer)
}
