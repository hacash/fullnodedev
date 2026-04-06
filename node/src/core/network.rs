use super::*;

impl NodeRuntime {
    pub fn start_network(&self, worker: Worker) {
        self.exited.store(false, Ordering::Relaxed);
        self.metrics.lock().unwrap().on_start();

        self.protocol.start_loop(&self.tasks, worker.fork());
        self.transport.start(worker);
    }

    pub fn stop_network(&self) {
        if self.exited.swap(true, Ordering::Relaxed) {
            return
        }
        self.metrics.lock().unwrap().on_exit();
        self.protocol.exit();
        self.transport.exit();
        self.engine.exit();
        println!("[Node] network exit. runtime_threads={}", self.running_task_count());
    }
}
