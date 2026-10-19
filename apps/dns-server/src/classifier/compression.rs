use async_compression::tokio::write::ZstdEncoder;
use color_eyre::eyre::Result;
use tokio::io::AsyncWriteExt;

/// Compress `bytes` with zstd.
pub async fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZstdEncoder::new(Vec::new());
    encoder.write_all(bytes).await?;
    encoder.shutdown().await?;

    Ok(encoder.into_inner())
}

/// Decompress zstd `bytes` produced by [`compress`].
#[cfg(test)]
pub async fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    use async_compression::tokio::bufread::ZstdDecoder;
    use tokio::io::AsyncReadExt;

    let mut decoder = ZstdDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).await?;

    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::{compress, decompress};

    #[tokio::test]
    async fn round_trips_arbitrary_bytes() {
        let original = b"<html>\xff\xfe not utf-8 \x00 <p>hello</p></html>".repeat(100);

        let compressed = compress(&original).await.unwrap();

        assert!(compressed.len() < original.len());
        assert_eq!(decompress(&compressed).await.unwrap(), original);
    }

    #[tokio::test]
    async fn round_trips_empty_input() {
        let compressed = compress(&[]).await.unwrap();

        assert!(decompress(&compressed).await.unwrap().is_empty());
    }
}
