use super::*;

#[derive(Clone)]
pub struct SyncState {
    pub active_peer: Option<PeerKey>,
    pub next_height: u64,
    pub remote_height: u64,
    pub updated_at: Instant,
}

pub struct SyncTracker {
    inner: Arc<StdMutex<Option<SyncState>>>,
}

impl SyncTracker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(StdMutex::new(None)),
        }
    }

    pub fn begin_or_refresh(&self, peer: &Arc<Peer>, starthei: u64, remote_height: u64) -> bool {
        let mut sync = self.inner.lock().unwrap();
        let now = Instant::now();
        if let Some(st) = sync.as_mut() {
            if let Some(pk) = st.active_peer {
                if pk != peer.key && now.duration_since(st.updated_at).as_secs() < 10 {
                    return false
                }
            }
            st.active_peer = Some(peer.key);
            st.next_height = starthei;
            st.remote_height = remote_height.max(st.remote_height);
            st.updated_at = now;
            return true
        }
        *sync = Some(SyncState {
            active_peer: Some(peer.key),
            next_height: starthei,
            remote_height,
            updated_at: now,
        });
        true
    }

    pub fn finish_if_done(&self, peer: &Arc<Peer>, next_height: u64, remote_height: u64) {
        let mut sync = self.inner.lock().unwrap();
        let Some(st) = sync.as_mut() else {
            return
        };
        if st.active_peer != Some(peer.key) {
            return
        }
        st.next_height = next_height;
        st.remote_height = remote_height;
        st.updated_at = Instant::now();
        if next_height > remote_height {
            *sync = None;
        }
    }

    pub fn clear_peer(&self, peer: &Arc<Peer>) {
        let mut sync = self.inner.lock().unwrap();
        if sync.as_ref().and_then(|s| s.active_peer) == Some(peer.key) {
            *sync = None;
        }
    }
}
