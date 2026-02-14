use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::*;

use async_broadcast::{broadcast, Sender, Receiver, Recv, TryRecvError};

struct ExitState {
    jobs: Mutex<isize>,
    notify: Condvar,
}

impl ExitState {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(0),
            notify: Condvar::new(),
        }
    }

    fn add_job(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        *jobs += 1;
        self.notify.notify_all();
    }

    fn end_job(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        *jobs -= 1;
        self.notify.notify_all();
    }

}


pub struct Worker {
    state: Arc<ExitState>,
    exiting: Arc<AtomicBool>,
    sender: Sender<()>,
    ended: Arc<AtomicBool>,
    receiver: Receiver<()>,
}

impl Clone for Worker {
    fn clone(&self) -> Self {
        self.fork()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.end();
    }
}

impl Worker {
    fn refresh_exit_signal(&self) {
        if self.exiting.load(Ordering::Acquire) {
            let _ = self.sender.try_broadcast(());
        }
    }

    pub fn fork(&self) -> Self {
        if self.ended.load(Ordering::Acquire) {
            panic!("cannot fork ended worker");
        }
        self.state.add_job();
        let worker = Self {
            state: self.state.clone(),
            exiting: self.exiting.clone(),
            sender: self.sender.clone(),
            ended: Arc::new(AtomicBool::new(false)),
            receiver: self.receiver.clone(),
        };
        worker.refresh_exit_signal();
        worker
    }

    fn end(&self) {
        if !self.ended.swap(true, Ordering::AcqRel) {
            self.state.end_job();
        }
    }

    pub fn wait(&mut self) -> Recv<'_, ()> {
        self.refresh_exit_signal();
        self.receiver.recv_direct()
    }

    pub fn quit(&mut self) -> bool {
        if self.exiting.load(Ordering::Acquire) {
            self.end();
            return true;
        }
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => false,
            _ => {
                self.end();
                true
            }
        }
    }

}



#[derive(Clone)]
pub struct Exiter {
    state: Arc<ExitState>,
    exiting: Arc<AtomicBool>,
    sender: Sender<()>,
    receiver: Receiver<()>,
}


impl Exiter {

    pub fn new() -> Self {
        let (s, r) = broadcast::<()>(5);
        Self {
            state: Arc::new(ExitState::new()),
            exiting: Arc::new(AtomicBool::new(false)),
            sender: s,
            receiver: r,
        }
    }

    pub fn worker(&self) -> Worker {
        self.state.add_job();
        let worker = Worker {
            state: self.state.clone(),
            exiting: self.exiting.clone(),
            sender: self.sender.clone(),
            ended: Arc::new(AtomicBool::new(false)),
            receiver: self.receiver.clone()
        };
        worker.refresh_exit_signal();
        worker
    }
    
    pub fn exit(&self) {
        if self.exiting.swap(true, Ordering::AcqRel) {
            return; // exit signal has already been sent
        }
        // broadcast to nitify all thread to quit
        #[cfg(not(target_family = "wasm"))]
        {
            let _ = self.sender.broadcast_blocking(());
        }
        #[cfg(target_family = "wasm")]
        {
            // `broadcast_blocking` is unavailable on wasm targets.
            let _ = self.sender.try_broadcast(());
        }
        self.state.notify.notify_all();
    }

    pub fn wait_exit_or_done(&self) -> bool {
        let mut jobs = self.state.jobs.lock().unwrap();
        while *jobs > 0 && !self.exiting.load(Ordering::Acquire) {
            jobs = self.state.notify.wait(jobs).unwrap();
        }
        self.exiting.load(Ordering::Acquire)
    }
    
    pub fn wait(&self) {
        let mut jobs = self.state.jobs.lock().unwrap();
        while *jobs > 0 {
            jobs = self.state.notify.wait(jobs).unwrap();
        }
    }


}
