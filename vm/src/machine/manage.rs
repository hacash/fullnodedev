

#[derive(Default)]
pub struct MachineManage {
    lock: Mutex<()>,
    // This pool is intentionally stored behind `UnsafeCell<Vec<Resoure>>` instead of
    // `Mutex<Vec<Resoure>>`. `MachineManage` is a global singleton shared across threads,
    // but `Resoure` is a single-thread VM runtime container and is not required to satisfy
    // the full `Send`/`Sync` bounds that `Mutex<Vec<Resoure>>` would impose transitively.
    //
    // Concurrency contract:
    // 1. Every access to `resoures` must hold `lock`.
    // 2. `assign()` removes one `Resoure` from the pool and transfers exclusive ownership
    //    to the caller (`MachineBox`), so the runtime is no longer shared with the pool.
    // 3. `reclaim()` returns that owned runtime back into the pool only after the caller is
    //    done using it.
    //
    // So the manager is thread-shared, but each rented `Resoure` stays thread-confined while
    // in use. The unsafe impls below rely on exactly this move-in/move-out discipline.
    resoures: UnsafeCell<Vec<Resoure>>,
    contract_cache: ContractCachePool,
}

// Safe because shared access only exposes the manager; the pooled `Resoure` values are touched
// only while holding `lock`, and each `assign()`/`reclaim()` moves ownership in or out of the pool.
unsafe impl Sync for MachineManage {}
unsafe impl Send for MachineManage {}

impl MachineManage {

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn with_resources<T>(&self, f: impl FnOnce(&mut Vec<Resoure>) -> T) -> T {
        let _lk = self.lock.lock().unwrap();
        // SAFETY: `lock` serializes every access to `resoures`, and callers only move whole
        // `Resoure` values in or out of the pool. No reference to the inner Vec escapes this
        // method beyond the lock scope.
        let res = unsafe { &mut *self.resoures.get() };
        f(res)
    }

    /* create a vm machine */
    pub fn assign(&self, hei: u64) -> MachineBox {
        let r = self.with_resources(|res| match res.pop() {
            Some(mut r) => {
                r.reset(hei);
                r
            }
            None => Resoure::create(hei),
        });
        MachineBox::from_resource(r)
    }

    pub fn reclaim(&self, mut r: Resoure) {
        r.reclaim();
        self.with_resources(|res| {
            res.push(r);
        });
    }

    pub fn contract_cache(&self) -> &ContractCachePool {
        &self.contract_cache
    }


}
