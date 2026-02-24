


/*
    Action hooker
*/

pub type FnActionHookFunc = fn(u16, _act: &dyn Any, _ctx: &mut dyn Context) -> Rerr ;

pub static mut ACTION_HOOK_FUNC: FnActionHookFunc = |_,_,_|Ok(());

pub fn action_hooker(f: FnActionHookFunc) {
    unsafe {
        ACTION_HOOK_FUNC = f;
    }
}

pub fn do_action_hook(kid: u16, _act: &dyn Any, _ctx: &mut dyn Context) -> Rerr {
    unsafe {
        ACTION_HOOK_FUNC(kid, _act, _ctx)
    }
}
