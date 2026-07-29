use super::manifest::InfoHash;

/// Local view of a media item's download/seed state. Populated and kept
/// live by a `SwarmEngine` adapter (see the backend README) — this type
/// itself is a pure snapshot, no I/O.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DownloadState {
    NotStarted,
    Downloading,
    Complete,
    Seeding,
}

#[derive(Clone, Debug)]
pub struct MediaItem {
    pub info_hash: InfoHash,
    pub state: DownloadState,
    /// 0.0..=1.0
    pub progress: f32,
}

impl MediaItem {
    pub fn new(info_hash: InfoHash) -> Self {
        Self {
            info_hash,
            state: DownloadState::NotStarted,
            progress: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_media_item_starts_at_zero_progress() {
        let item = MediaItem::new(InfoHash::from_bytes([0; 20]));

        assert_eq!(item.state, DownloadState::NotStarted);
        assert_eq!(item.progress, 0.0);
    }
}
