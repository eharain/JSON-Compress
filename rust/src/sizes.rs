//! Byte measurement, and gzip where the feature is on.
//!
//! What actually goes over the wire is UTF-8 bytes, so that is what these count.

/// The length of a string in UTF-8 bytes - what goes over the wire, as opposed
/// to the number of characters in it.
pub fn byte_length(text: &str) -> usize {
    text.len()
}

/// Gzips a document.
///
/// The header carries no timestamp, so the same input always gives the same
/// bytes and a size can be compared across runs. The level is the library
/// default, which is what browsers use for `CompressionStream`, so a figure
/// measured here is comparable with one measured in the JavaScript
/// implementation.
///
/// The standard library has no deflate, so this is the one thing in the crate
/// that needs a dependency, and it is behind the `gzip` feature.
#[cfg(feature = "gzip")]
#[cfg_attr(docsrs, doc(cfg(feature = "gzip")))]
pub fn gzip_bytes(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Reverses [`gzip_bytes`].
#[cfg(feature = "gzip")]
#[cfg_attr(docsrs, doc(cfg(feature = "gzip")))]
pub fn gunzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut out = Vec::new();
    GzDecoder::new(data).read_to_end(&mut out)?;
    Ok(out)
}

/// How many bytes a document takes once gzipped, which is the figure
/// [`crate::Report`] carries.
#[cfg(feature = "gzip")]
#[cfg_attr(docsrs, doc(cfg(feature = "gzip")))]
pub fn gzip_size(data: &[u8]) -> usize {
    gzip_bytes(data).map_or(0, |out| out.len())
}
