use crate::action;

fn ctx_action_call(this: &mut dyn Context, k: u16, b: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
    // create
    let body = vec![k.to_be_bytes().to_vec(), b].concat();
    let (action, used) = action::action_create(&body).into_xret()?;
    if used != body.len() {
        return xerrf!(
            "action parse length mismatch: consumed {} but body length is {}",
            used,
            body.len()
        );
    }
    if this.env().chain.fast_sync {
        action::check_action_scope(ExecFrom::Call, action.as_ref()).into_xret()?;
    }
    action::check_action_ast_tree_depth(action.as_ref()).into_xret()?;
    // ACTION payload actions are runtime-created and not part of tx.actions.
    // Keep runtime req_sign checks here; tx.main signature is already verified in tx.execute().
    let mut seen = HashSet::new();
    for ptr in action.req_sign() {
        let adr = this.addr(&ptr).into_xret()?;
        if !seen.insert(adr) {
            continue;
        }
        if adr.is_privakey() {
            this.check_sign(&adr).into_xret()?;
        }
    }
    // Runtime-created actions always execute in CALL context.
    let (gas, res) = with_exec_from(this, ExecFrom::Call, |ctx| action.execute(ctx))?;
    let gas = apply_extra9_surcharge(action.extra9(), gas);
    Ok((gas, res))
}
