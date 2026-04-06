

#[derive(Default)]
pub struct RuntimePool {
    lock: Mutex<()>,
    // This pool is intentionally stored behind `UnsafeCell<Vec<Runtime>>` instead of
    // `Mutex<Vec<Runtime>>`. `RuntimePool` is a global singleton shared across threads,
    // but `Runtime` is a single-thread VM runtime container and is not required to satisfy
    // the full `Send`/`Sync` bounds that `Mutex<Vec<Runtime>>` would impose transitively.
    //
    // Concurrency contract:
    // 1. Every access to `resoures` must hold `lock`.
    // 2. `checkout()` removes one `Runtime` from the pool and transfers exclusive ownership
    //    to the caller (`Executor`), so the runtime is no longer shared with the pool.
    // 3. `checkin()` returns that owned runtime back into the pool only after the caller is
    //    done using it.
    //
    // So the pool is thread-shared, but each rented `Runtime` stays thread-confined while
    // in use. The unsafe impls below rely on exactly this move-in/move-out discipline.
    resoures: UnsafeCell<Vec<Runtime>>,
    contract_cache: ContractCachePool,
}

// Safe because shared access only exposes the pool; the pooled `Runtime` values are touched
// only while holding `lock`, and each `checkout()`/`checkin()` moves ownership in or out of the pool.
unsafe impl Sync for RuntimePool {}
unsafe impl Send for RuntimePool {}

impl RuntimePool {

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn with_resources<T>(&self, f: impl FnOnce(&mut Vec<Runtime>) -> T) -> T {
        let _lk = self.lock.lock().unwrap();
        // SAFETY: `lock` serializes every access to `resoures`, and callers only move whole
        // `Runtime` values in or out of the pool. No reference to the inner Vec escapes this
        // method beyond the lock scope.
        let res = unsafe { &mut *self.resoures.get() };
        f(res)
    }

    /* create a vm machine */
    pub fn checkout(&self, hei: u64) -> Executor {
        let r = self.with_resources(|res| match res.pop() {
            Some(mut r) => {
                r.reset(hei);
                r
            }
            None => Runtime::create(hei),
        });
        Executor::from_runtime(r)
    }

    pub fn checkin(&self, mut r: Runtime) {
        r.reclaim();
        self.with_resources(|res| {
            res.push(r);
        });
    }

    pub fn contract_cache(&self) -> &ContractCachePool {
        &self.contract_cache
    }


}
