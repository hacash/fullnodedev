use basis::interface::VM;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

pub struct CounterMockVm {
    counter: Arc<AtomicI64>,
}

impl CounterMockVm {
    pub fn new(counter: Arc<AtomicI64>) -> Self {
        Self { counter }
    }

    pub fn create() -> (Box<dyn VM>, Arc<AtomicI64>) {
        let counter = Arc::new(AtomicI64::new(0));
        (Box::new(Self::new(counter.clone())), counter)
    }
}

impl VM for CounterMockVm {
    fn usable(&self) -> bool {
        true
    }

    fn snapshot_volatile(&self) -> Box<dyn Any> {
        Box::new(self.counter.load(Ordering::SeqCst))
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        match snap.downcast::<i64>() {
            Ok(c) => self.counter.store(*c, Ordering::SeqCst),
            Err(_) => panic!("CounterMockVm::restore_volatile expects i64 snapshot"),
        }
    }
}

pub fn new_counter_vm() -> (Box<dyn VM>, Arc<AtomicI64>) {
    CounterMockVm::create()
}
