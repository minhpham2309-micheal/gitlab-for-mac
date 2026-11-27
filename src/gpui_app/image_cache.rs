use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use gpui_kit::{App, Global, Image, ImageFormat};

use crate::gitlab::GitLab;
use crate::runtime::spawn_tokio;

const MAX_ENTRIES: usize = 128;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;

type CacheKey = (String, Option<u64>, String);

#[derive(Clone)]
pub enum CacheEntry {
    Pending,
    Ready(Arc<Image>),
    Failed(String),
}

struct CacheInner {
    generation: u64,
    entries: HashMap<CacheKey, (CacheEntry, usize)>,
    order: VecDeque<CacheKey>,
    total_bytes: usize,
}

#[derive(Clone)]
pub struct ImageCache {
    inner: Arc<RwLock<CacheInner>>,
}

impl Global for ImageCache {}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(CacheInner {
                generation: 1,
                entries: HashMap::new(),
                order: VecDeque::new(),
                total_bytes: 0,
            })),
        }
    }

    pub fn init(cx: &mut App) {
        if cx.try_global::<Self>().is_none() {
            cx.set_global(Self::new());
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.generation = guard.generation.wrapping_add(1);
            guard.entries.clear();
            guard.order.clear();
            guard.total_bytes = 0;
        }
    }

    pub fn lookup(
        &self,
        host_origin: &str,
        project_id: Option<u64>,
        raw_src: &str,
    ) -> Option<CacheEntry> {
        let key = (host_origin.to_string(), project_id, raw_src.to_string());
        let guard = self.inner.read().ok()?;
        guard.entries.get(&key).map(|(e, _)| e.clone())
    }

    pub fn get_or_fetch(
        &self,
        client: &GitLab,
        raw_src: &str,
        project_id: Option<u64>,
        cx: &mut App,
    ) -> Option<Arc<Image>> {
        let origin_key = client.origin().ascii_serialization();
        let key = (origin_key, project_id, raw_src.to_string());
        if let Ok(guard) = self.inner.read()
            && let Some((entry, _)) = guard.entries.get(&key)
        {
            return match entry {
                CacheEntry::Ready(img) => Some(img.clone()),
                CacheEntry::Pending | CacheEntry::Failed(_) => None,
            };
        }
        let req_gen = {
            let guard = self.inner.write().ok()?;
            if let Some((entry, _)) = guard.entries.get(&key) {
                return match entry {
                    CacheEntry::Ready(img) => Some(img.clone()),
                    CacheEntry::Pending | CacheEntry::Failed(_) => None,
                };
            }
            guard.generation
        };
        self.insert_entry(req_gen, key.clone(), CacheEntry::Pending, 0);
        let cache = self.clone();
        let client = client.clone();
        let src = raw_src.to_string();
        cx.spawn(async move |cx| {
            let fetched = spawn_tokio(async move {
                client
                    .fetch_media(&src, project_id)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
            match fetched {
                Ok(Ok(img)) => {
                    if let Some(fmt) = detect_format(&img.content_type, &img.bytes) {
                        let byte_len = img.bytes.len();
                        let decoded = Arc::new(Image::from_bytes(fmt, img.bytes));
                        cache.insert_entry(req_gen, key, CacheEntry::Ready(decoded), byte_len);
                    } else {
                        cache.insert_entry(
                            req_gen,
                            key,
                            CacheEntry::Failed("unsupported image".into()),
                            0,
                        );
                    }
                }
                Ok(Err(err)) | Err(err) => {
                    cache.insert_entry(req_gen, key, CacheEntry::Failed(err), 0);
                }
            }
            cx.update(|cx| cx.refresh_windows());
        })
        .detach();
        None
    }

    fn insert_entry(&self, expected_gen: u64, key: CacheKey, entry: CacheEntry, bytes: usize) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        if guard.generation != expected_gen {
            return;
        }
        if let Some((_, old_bytes)) = guard.entries.insert(key.clone(), (entry, bytes)) {
            guard.total_bytes = guard.total_bytes.saturating_sub(old_bytes);
            if let Some(pos) = guard.order.iter().position(|k| k == &key) {
                guard.order.remove(pos);
            }
        }
        guard.order.push_back(key);
        guard.total_bytes = guard.total_bytes.saturating_add(bytes);
        while guard.entries.len() > MAX_ENTRIES || guard.total_bytes > MAX_CACHE_BYTES {
            let Some(oldest) = guard.order.pop_front() else {
                break;
            };
            if let Some((_, evicted_bytes)) = guard.entries.remove(&oldest) {
                guard.total_bytes = guard.total_bytes.saturating_sub(evicted_bytes);
            }
        }
    }
}

fn detect_format(content_type: &str, bytes: &[u8]) -> Option<ImageFormat> {
    if content_type.contains("png") || bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(ImageFormat::Png)
    } else if content_type.contains("jpeg")
        || content_type.contains("jpg")
        || bytes.starts_with(&[0xFF, 0xD8])
    {
        Some(ImageFormat::Jpeg)
    } else if content_type.contains("gif") || bytes.starts_with(b"GIF8") {
        Some(ImageFormat::Gif)
    } else if content_type.contains("webp")
        || (bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
    {
        Some(ImageFormat::Webp)
    } else if content_type.contains("svg") {
        Some(ImageFormat::Svg)
    } else {
        None
    }
}
