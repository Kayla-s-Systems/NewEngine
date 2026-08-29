use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use newengine_assets_api::{AssetRawRangeRequest, AssetServiceClient};

#[derive(Debug, Default)]
pub(crate) struct StreamingAssetIoStats {
    range_requests: AtomicU64,
    compressed_bytes_fetched: AtomicU64,
    cache_hits: AtomicU64,
}

impl StreamingAssetIoStats {
    #[inline]
    pub(crate) fn range_requests(&self) -> u64 {
        self.range_requests.load(Ordering::Relaxed)
    }

    #[inline]
    pub(crate) fn compressed_bytes_fetched(&self) -> u64 {
        self.compressed_bytes_fetched.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }
}

pub(crate) struct RangedAssetReader {
    assets: AssetServiceClient,
    logical_path: String,
    position: u64,
    total_len: Option<u64>,
    chunk_bytes: u32,
    max_cache_chunks: usize,
    cache: BTreeMap<u64, Arc<[u8]>>,
    lru: VecDeque<u64>,
    stats: Arc<StreamingAssetIoStats>,
}

impl RangedAssetReader {
    pub(crate) fn new(
        assets: AssetServiceClient,
        logical_path: String,
        chunk_bytes: u32,
        cache_bytes: u32,
    ) -> Self {
        let chunk_bytes = chunk_bytes.clamp(4 * 1024, 1024 * 1024);
        let cache_bytes = cache_bytes
            .clamp(16 * 1024, 16 * 1024 * 1024)
            .max(chunk_bytes);
        let max_cache_chunks = ((cache_bytes as usize) / (chunk_bytes as usize)).max(1);
        Self {
            assets,
            logical_path,
            position: 0,
            total_len: None,
            chunk_bytes,
            max_cache_chunks,
            cache: BTreeMap::new(),
            lru: VecDeque::new(),
            stats: Arc::new(StreamingAssetIoStats::default()),
        }
    }

    #[inline]
    pub(crate) fn stats(&self) -> Arc<StreamingAssetIoStats> {
        Arc::clone(&self.stats)
    }

    fn touch_lru(&mut self, chunk_index: u64) {
        if let Some(position) = self.lru.iter().position(|value| *value == chunk_index) {
            self.lru.remove(position);
        }
        self.lru.push_back(chunk_index);
        while self.lru.len() > self.max_cache_chunks {
            if let Some(evicted) = self.lru.pop_front() {
                self.cache.remove(&evicted);
            }
        }
    }

    fn fetch_chunk(&mut self, chunk_index: u64) -> io::Result<Arc<[u8]>> {
        if let Some(bytes) = self.cache.get(&chunk_index).cloned() {
            self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
            self.touch_lru(chunk_index);
            return Ok(bytes);
        }
        let offset = chunk_index
            .checked_mul(self.chunk_bytes as u64)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "stream chunk offset overflow")
            })?;
        let request =
            AssetRawRangeRequest::new(self.logical_path.clone(), offset, self.chunk_bytes);
        let response = self.assets.raw_range_v1(&request).map_err(|error| {
            io::Error::other(format!(
                "engine.assets range read failed path='{}' offset={} len={}: {}",
                self.logical_path, offset, self.chunk_bytes, error
            ))
        })?;
        self.stats.range_requests.fetch_add(1, Ordering::Relaxed);
        self.stats
            .compressed_bytes_fetched
            .fetch_add(response.bytes.len() as u64, Ordering::Relaxed);
        if let Some(total_len) = self.total_len {
            if total_len != response.total_len {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "stream asset length changed during playback: {} -> {}",
                        total_len, response.total_len
                    ),
                ));
            }
        } else {
            self.total_len = Some(response.total_len);
        }
        let expected_offset = offset.min(response.total_len);
        if response.offset != expected_offset {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "stream range response offset mismatch: got {} expected {}",
                    response.offset, expected_offset
                ),
            ));
        }
        let bytes: Arc<[u8]> = Arc::from(response.bytes);
        self.cache.insert(chunk_index, Arc::clone(&bytes));
        self.touch_lru(chunk_index);
        Ok(bytes)
    }

    fn ensure_total_len(&mut self) -> io::Result<u64> {
        if let Some(total_len) = self.total_len {
            return Ok(total_len);
        }
        let _ = self.fetch_chunk(0)?;
        Ok(self.total_len.unwrap_or(0))
    }
}

impl Read for RangedAssetReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let total_len = self.ensure_total_len()?;
        if self.position >= total_len {
            return Ok(0);
        }

        let mut written = 0usize;
        while written < out.len() && self.position < total_len {
            let chunk_index = self.position / self.chunk_bytes as u64;
            let chunk_offset = (self.position % self.chunk_bytes as u64) as usize;
            let chunk = self.fetch_chunk(chunk_index)?;
            if chunk_offset >= chunk.len() {
                if self.position >= total_len {
                    break;
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "engine.assets ranged stream returned an empty interior chunk",
                ));
            }
            let available = chunk.len() - chunk_offset;
            let count = available.min(out.len() - written);
            out[written..written + count]
                .copy_from_slice(&chunk[chunk_offset..chunk_offset + count]);
            written += count;
            self.position = self.position.saturating_add(count as u64);
        }
        Ok(written)
    }
}

impl Seek for RangedAssetReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let total_len = self.ensure_total_len()?;
        let next = match position {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
            SeekFrom::End(delta) => i128::from(total_len) + i128::from(delta),
        };
        if next < 0 || next > i128::from(u64::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "stream seek target is outside u64 range",
            ));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_budget_is_derived_from_chunk_budget() {
        // This test deliberately avoids a service call; it proves the local cache cannot
        // retain more chunks than the authored compressed byte budget permits.
        let host = newengine_plugin_host::default_host_api();
        let reader = RangedAssetReader::new(
            AssetServiceClient::new(host),
            "shared/audio/test.ogg".to_owned(),
            64 * 1024,
            256 * 1024,
        );
        assert_eq!(reader.max_cache_chunks, 4);
    }
}
