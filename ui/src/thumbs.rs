//! Thumbnails, made off the UI thread and kept as textures for the tiles on screen.
//!
//! A tile asks for its picture every frame it is visible; the newest requests are served first,
//! so scrolling fast never queues thousands of decodes. Pictures made from originals (not the
//! catalogue's ready-made previews) are cached on disk, so the next start is instant.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use egui::{ColorImage, TextureHandle, TextureOptions};

/// Longest side of a thumbnail, in pixels.
pub const SIZE: u32 = 256;
/// Originals larger than this are not decoded for a thumbnail.
const MAX_SOURCE_BYTES: u64 = 120 << 20;
/// Textures kept in memory.
const MAX_TEXTURES: usize = 1200;
/// Decoding threads.
const WORKERS: usize = 3;

struct Job {
    key: String,
    source: PathBuf,
    /// Whether to write the result to the disk cache (originals, not ready-made previews).
    cache: bool,
}

struct Queue {
    jobs: Mutex<VecDeque<Job>>,
    ready: Condvar,
}

pub struct Thumbs {
    queue: Arc<Queue>,
    results: Receiver<(String, Option<ColorImage>)>,
    textures: HashMap<String, TextureHandle>,
    /// Keys asked for and not answered yet.
    pending: HashSet<String>,
    /// Keys that have no picture (unreadable or not an image).
    missing: HashSet<String>,
    /// Use order, oldest first, to drop textures past the limit.
    order: VecDeque<String>,
}

impl Thumbs {
    pub fn new() -> Thumbs {
        let queue = Arc::new(Queue { jobs: Mutex::new(VecDeque::new()), ready: Condvar::new() });
        let (tx, rx) = channel();
        let cache_dir = dirs_cache();
        for n in 0..WORKERS {
            let queue = queue.clone();
            let tx: Sender<(String, Option<ColorImage>)> = tx.clone();
            let cache_dir = cache_dir.clone();
            let _ = std::thread::Builder::new().name(format!("library-thumbs-{n}")).spawn(move || worker(&queue, &tx, cache_dir.as_deref()));
        }
        Thumbs {
            queue,
            results: rx,
            textures: HashMap::new(),
            pending: HashSet::new(),
            missing: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    /// Moves finished pictures into textures. Call once per frame.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut got = false;
        while let Ok((key, image)) = self.results.try_recv() {
            self.pending.remove(&key);
            got = true;
            match image {
                Some(img) => {
                    let tex = ctx.load_texture(format!("library:{key}"), img, TextureOptions::LINEAR);
                    self.textures.insert(key.clone(), tex);
                    self.order.push_back(key);
                }
                None => {
                    self.missing.insert(key);
                }
            }
        }
        while self.textures.len() > MAX_TEXTURES {
            let Some(old) = self.order.pop_front() else { break };
            self.textures.remove(&old);
        }
        if got || !self.pending.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    /// The texture for `key`, asking for it from `source` when there is none yet.
    pub fn get(&mut self, key: &str, source: Option<PathBuf>, from_original: bool) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(key) {
            return Some(t.clone());
        }
        if self.missing.contains(key) || self.pending.contains(key) {
            return None;
        }
        let source = source?;
        self.pending.insert(key.to_string());
        let mut jobs = self.queue.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        jobs.push_front(Job { key: key.to_string(), source, cache: from_original });
        // Requests for tiles scrolled away long ago are dropped.
        while jobs.len() > 400 {
            if let Some(dropped) = jobs.pop_back() {
                self.pending.remove(&dropped.key);
            }
        }
        drop(jobs);
        self.queue.ready.notify_one();
        None
    }

    /// Whether `key` is known to have no picture.
    pub fn is_missing(&self, key: &str) -> bool {
        self.missing.contains(key)
    }
}

impl Default for Thumbs {
    fn default() -> Self {
        Thumbs::new()
    }
}

fn dirs_cache() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("Oracle").join("library-thumbs"))
}

fn worker(queue: &Queue, tx: &Sender<(String, Option<ColorImage>)>, cache_dir: Option<&Path>) {
    loop {
        let job = {
            let mut jobs = queue.jobs.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if let Some(j) = jobs.pop_front() {
                    break j;
                }
                jobs = queue.ready.wait(jobs).unwrap_or_else(PoisonError::into_inner);
            }
        };
        let image = std::panic::catch_unwind(|| make(&job, cache_dir)).ok().flatten();
        if tx.send((job.key, image)).is_err() {
            return;
        }
    }
}

/// The cache file for a source: a hash of its path, size and modification time.
fn cache_file(dir: &Path, source: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(source).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    Some(dir.join(format!("{:016x}.png", h.finish())))
}

fn make(job: &Job, cache_dir: Option<&Path>) -> Option<ColorImage> {
    let cached = if job.cache { cache_dir.and_then(|d| cache_file(d, &job.source)) } else { None };
    if let Some(c) = &cached
        && let Ok(img) = image::open(c)
    {
        return Some(to_color_image(img.to_rgba8()));
    }
    let meta = std::fs::metadata(&job.source).ok()?;
    if meta.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let ext = job.source.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let rgba = if ext == "svg" || ext == "svgz" { render_svg(&job.source)? } else { image::open(&job.source).ok()?.thumbnail(SIZE, SIZE).to_rgba8() };
    if let Some(c) = cached {
        if let Some(dir) = c.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = rgba.save(&c);
    }
    Some(to_color_image(rgba))
}

fn render_svg(path: &Path) -> Option<image::RgbaImage> {
    use resvg::{tiny_skia, usvg};
    let data = std::fs::read(path).ok()?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let (w, h) = (size.width().max(1.0), size.height().max(1.0));
    let scale = SIZE as f32 / w.max(h);
    let (pw, ph) = (((w * scale).round() as u32).max(1), ((h * scale).round() as u32).max(1));
    let mut pixmap = tiny_skia::Pixmap::new(pw, ph)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // tiny-skia stores premultiplied alpha; egui wants it straight.
    let mut out = image::RgbaImage::new(pw, ph);
    for (dst, src) in out.pixels_mut().zip(pixmap.pixels()) {
        let c = src.demultiply();
        *dst = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Some(out)
}

fn to_color_image(rgba: image::RgbaImage) -> ColorImage {
    let size = [rgba.width() as usize, rgba.height() as usize];
    ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}
