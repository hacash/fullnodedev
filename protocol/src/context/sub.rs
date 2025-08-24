
pub struct EmptyState {}
impl State for EmptyState {}



/*
* return old and parent state
*/
fn ctx_state_fork_sub(ctx: &mut dyn Context) -> Box<dyn State> {
    let nil = Box::new(EmptyState{});
    let mut old: Arc<dyn State> = ctx.state_replace(nil).into();
    let sub = old.fork_sub(Arc::downgrade(&old));
    ctx.state_replace(sub);
    // arc => box
    Arc::get_mut(&mut old).map(|p| {
        unsafe { Box::from_raw(p as *mut dyn State) }
    }).unwrap()
}


/*
*/
fn ctx_state_merge_sub(ctx: &mut dyn Context, mut old: Box<dyn State>) {
    let nil = Box::new(EmptyState{});
    let sub = ctx.state_replace(nil);
    old.merge_sub(sub);
    ctx.state_replace(old);
}

