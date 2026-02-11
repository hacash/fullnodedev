use crate::action;

fn ctx_action_call(this: &mut ContextInst, k: u16, b: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
    // create
    let body = vec![k.to_be_bytes().to_vec(), b].concat();
    let (action, _) = action::action_create(&body)?;
    // EXTACTION payload actions are runtime-created and not part of tx.actions.
    // Keep runtime req_sign checks here; tx.main signature is already verified in tx.execute().
    let mut seen = HashSet::new();
    for ptr in action.req_sign() {
        let adr = this.addr(&ptr)?;
        if !seen.insert(adr) {
            continue;
        }
        if adr.is_privakey() {
            this.check_sign(&adr)?;
        }
    }
    let (mut gas, res) = action.execute(this)?;
    // burn_90 OR rule: if either the tx or the action is burn_90, apply 10x gas multiplier.
    // This is the single place where burn_90 gas penalty is applied — action.execute()
    // returns base gas (size only), and we multiply here.
    if this.tx().burn_90() || action.burn_90() {
        gas = gas.saturating_mul(10);
    }
    Ok((gas, res))
}
