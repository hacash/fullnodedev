

const ISRT_STAT_IDLE: usize = 0;
const ISRT_STAT_DISCOVER: usize = 1;
const ISRT_STAT_SYNCING: usize = 2;

struct InsertingLock<'a> {
    mark: &'a std::sync::atomic::AtomicUsize,
}

impl Drop for InsertingLock<'_> {
    fn drop(&mut self) {
        // Release: publish all writes in critical section before unlocking.
        self.mark.store(ISRT_STAT_IDLE, Ordering::Release);
    }
}

fn inserting_lock<'a>(eng: &'a ChainEngine, change_to_stat: usize, busy_tip: &str) -> Ret<InsertingLock<'a>> {
    loop {
        match eng.inserting.compare_exchange(ISRT_STAT_IDLE, change_to_stat, Ordering::AcqRel, Ordering::Acquire) {
            Ok(ISRT_STAT_IDLE) => break,
            Err(ISRT_STAT_DISCOVER) => {
                sleep(Duration::from_millis(100));
                continue;
            },
            Err(ISRT_STAT_SYNCING) => {
                return errf!("{}", busy_tip)
            }
            _ => never!()
        }
    }
    Ok(InsertingLock{ mark: &eng.inserting })
}

fn sync_warning(e: String) -> Rerr {
    errf!("\n[Block Sync Warning] {}\n", e)
}
