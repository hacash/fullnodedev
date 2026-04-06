use super::*;

pub struct TaskGroup {
    running: AtomicUsize,
}

impl TaskGroup {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            running: AtomicUsize::new(0),
        })
    }

    pub fn spawn_thread<F>(self: &Arc<Self>, name: &'static str, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.running.fetch_add(1, Ordering::Relaxed);
        let this = self.clone();
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                f();
                this.running.fetch_sub(1, Ordering::Relaxed);
            })
            .expect("cannot spawn runtime thread");
    }

    pub fn running(&self) -> usize {
        self.running.load(Ordering::Relaxed)
    }
}
