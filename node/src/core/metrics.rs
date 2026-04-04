#[derive(Default)]
pub struct RuntimeMetrics {
    pub start_count: u64,
    pub exit_count: u64,
}

impl RuntimeMetrics {
    pub fn on_start(&mut self) {
        self.start_count += 1;
    }

    pub fn on_exit(&mut self) {
        self.exit_count += 1;
    }
}
