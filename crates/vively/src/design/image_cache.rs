use std::{collections::HashMap, sync::Arc};

use gpui_kit::{
    App, AppContext as _, Entity, ImageCache, ImageCacheError, ImageCacheItem, RenderImage,
    Resource, Window, hash,
};

pub const DECODED_IMAGE_BUDGET: usize = 32 * 1024 * 1024;

struct Entry {
    item: ImageCacheItem,
    last_frame: u64,
}

pub struct BoundedImageCache {
    entries: HashMap<u64, Entry>,
    frame: u64,
    budget_bytes: usize,
}

impl BoundedImageCache {
    pub fn new(budget_bytes: usize, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            entries: HashMap::new(),
            frame: 0,
            budget_bytes,
        });
        cx.observe_release(&entity, |cache, cx| {
            for (_, entry) in cache.entries.drain() {
                if let Some(Ok(image)) = entry.item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        entity
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut App) {
        for (_, entry) in self.entries.drain() {
            if let Some(Ok(image)) = entry.item.get() {
                cx.drop_image(image, Some(window));
            }
        }
    }

    fn decoded_bytes(&self) -> usize {
        self.entries
            .values()
            .filter_map(|entry| loaded_size(&entry.item))
            .sum()
    }

    pub fn begin_frame(&mut self, window: &mut Window, cx: &mut App) {
        let previous = self.frame;
        self.frame += 1;

        let mut total = self.decoded_bytes();
        if total <= self.budget_bytes {
            return;
        }

        let mut stale: Vec<(u64, u64, usize)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_frame < previous)
            .filter_map(|(key, entry)| {
                loaded_size(&entry.item).map(|size| (*key, entry.last_frame, size))
            })
            .collect();
        stale.sort_unstable_by_key(|(_, last_frame, _)| *last_frame);

        for (key, _, size) in stale {
            if total <= self.budget_bytes {
                break;
            }
            if let Some(entry) = self.entries.remove(&key)
                && let Some(Ok(image)) = entry.item.get()
            {
                cx.drop_image(image, Some(window));
            }
            total = total.saturating_sub(size);
        }
    }
}

fn loaded_size(item: &ImageCacheItem) -> Option<usize> {
    let image = item.get()?.ok()?;
    Some(render_image_bytes(&image))
}

fn render_image_bytes(image: &RenderImage) -> usize {
    (0..image.frame_count())
        .map(|frame| {
            let size = image.size(frame);
            size.width.0.max(0) as usize * size.height.0.max(0) as usize * 4
        })
        .sum()
}

impl ImageCache for BoundedImageCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let frame = self.frame;
        let entry = self.entries.entry(hash(resource)).or_insert_with(|| Entry {
            item: ImageCacheItem::new(resource, cx),
            last_frame: frame,
        });
        entry.last_frame = frame;
        entry.item.use_image(window)
    }
}
