use std::{sync::*, thread::sleep};

use async_broadcast::{broadcast, Sender, Receiver, Recv, TryRecvError};

type JobCount = Arc<Mutex<isize>>;


pub struct Worker {
    jobs: Arc<Mutex<Option<JobCount>>>,
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

    pub fn fork(&self) -> Self {
        let mut jbw =  self.jobs.lock().unwrap();
        let Some(jobs) = jbw.as_mut() else {
            panic!("cannot fork worker on end one");
        };
        let mut jbn = jobs.lock().unwrap();
        *jbn += 1;
        Self {
            jobs: Arc::new(Some(jobs.clone()).into()),
            receiver: self.receiver.clone(),
        }
    }

    pub fn end(&self) {
        if let Some(jobs) = self.jobs.lock().unwrap().take() {
            let mut jbn = jobs.lock().unwrap();
            *jbn -= 1;
        }
    }

    pub fn wait(&mut self) -> Recv<'_, ()> {
        self.receiver.recv_direct()
    }

    pub fn quit(&mut self) -> bool {
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => false,
            _ => {
                self.end();
                true
            }
        }
    }

}



#[allow(dead_code)]
#[derive(Clone)]
pub struct Exiter {
    jobs: JobCount,
    sender: Sender<()>,
    receiver: Receiver<()>,
}


impl Exiter {

    pub fn new() -> Self {
        let (s, r) = broadcast::<()>(5);
        Self {
            jobs: Arc::default(),
            sender: s,
            receiver: r,
        }
    }

    pub fn worker(&self) -> Worker {
        let mut jobs = self.jobs.lock().unwrap();
        *jobs += 1;
        Worker {
            jobs: Arc::new(Some(self.jobs.clone()).into()),
            receiver: self.receiver.clone()
        }
    }
    
    pub fn exit(&self) {
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
    }
    
    pub fn wait(self) {
        loop {
            sleep(Duration::from_millis(333));
            let j = self.jobs.lock().unwrap();
            // println!("Exiter::wait, jobs={}", *j);
            if *j <= 0 {
                break; // exit all
            }
        }
    }


}
