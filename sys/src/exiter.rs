use std::{sync::*, thread::sleep};

use async_broadcast::{broadcast, Sender, Receiver, Recv, TryRecvError};

#[derive(Clone)]
pub struct Worker {
    jobs: Arc<Mutex<isize>>,
    receiver: Receiver<()>,
}

impl Worker {

    pub fn exit(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        *jobs -= 1;
    }

    pub fn wait_exit(&mut self) -> Recv<'_, ()> {
        self.receiver.recv_direct()
    }

    pub fn check_exit(&mut self) -> bool {
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => false,
            _ => {
                self.exit();
                true
            }
        }
    }

}



#[allow(dead_code)]
#[derive(Clone)]
pub struct Exiter {
    jobs: Arc<Mutex<isize>>,
    sender: Sender<()>,
    receiver: Receiver<()>,
}



impl Exiter {

    pub fn new() -> Self {
        let (s, r) = broadcast::<()>(5);
        let clts = s.clone();
        ctrlc::set_handler(move||{ 
            let _ = clts.broadcast_blocking(()); 
        }).unwrap();
        Self {
            jobs: Arc::default(),
            sender: s,
            receiver: r,
        }
    }

    pub fn work(&self) -> Worker {
        Worker {
            jobs: self.jobs.clone(),
            receiver: self.receiver.clone()
        }
    }
    
    
    pub fn wait(self) {
        loop {
            sleep(Duration::from_millis(333));
            let j = self.jobs.lock().unwrap();
            if *j <= 0 {
                break; // exit all
            }
        }
    }


}


/*

use std::sync::Arc;
use tokio::sync::broadcast::{self, Receiver, Sender};


#[allow(dead_code)]
#[derive(Clone)]
pub struct Exiter {
    closech: Arc<Receiver<bool>>,
    closechtx: Sender<bool>,
}


impl Exiter {

    pub fn new() -> Self {
        let (closetx, closerx) = broadcast::channel(4);
        Self {
            closech: closerx.into(),
            closechtx: closetx,
        }
    }

    pub fn sender(&self) -> Sender<bool> {
        self.closechtx.clone()
    }

    pub fn signal(&self) -> Receiver<bool> {
        self.closechtx.subscribe()
    }

    pub fn exit(&self) {
        let _ = self.closechtx.send(true);
    }


}


*/


