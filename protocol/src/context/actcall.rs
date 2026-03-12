use crate::action;

fn ctx_action_call(this: &mut ContextInst, k: u16, b: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
    // create
    let body = vec![k.to_be_bytes().to_vec(), b].concat();
    let (action, used) = action::action_create(&body).into_xret()?;
    if used != body.len() {
        return xerrf!(
            "action parse length mismatch: used {} but total {}",
            used,
            body.len()
        )
    }
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
    // Explicit call origin for level checks: this path is runtime ACTION.
    let old_from = this.action_exec_from();
    this.action_exec_from_set(ActExecFrom::ActionCall);
    let exec_res = action.execute(this);
    this.action_exec_from_set(old_from);
    let (gas, res) = exec_res?;
    let gas = apply_burn90_multiplier(this.tx().burn_90(), action.burn_90(), gas);
    Ok((gas, res))
}
