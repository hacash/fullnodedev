/*
    Action hooker
*/

pub type FnActionHookFunc = fn(u16, _act: &dyn Any, _ctx: &mut dyn Context) -> Rerr;

pub fn do_action_hook(kid: u16, _act: &dyn Any, _ctx: &mut dyn Context) -> Rerr {
    let registry = get_registry()?;
    for hook in &registry.action_hooks {
        hook(kid, _act, _ctx)?;
    }
    Ok(())
}
