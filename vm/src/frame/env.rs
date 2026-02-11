

#[allow(dead_code)]
pub struct ExecEnv<'a> {
    pub ctx: &'a mut dyn Context,
    pub gas: &'a mut i64,
}

