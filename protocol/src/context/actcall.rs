use crate::action;

fn ctx_action_call(this: &mut ContextInst, k: u16, b: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
    // create
    let body = vec![k.to_be_bytes().to_vec(), b].concat();
    let (action, _) = action::action_create(&body)?;
    // runtime signature gating: actions called from VM (EXTACTION/EXTVIEW/EXTENV) are not part of tx.actions,
    // so they must self-check required signatures here.
    for ptr in action.req_sign() {
        let adr = this.addr(&ptr)?;
        if adr.is_privakey() {
            this.check_sign(&adr)?;
        }
    }
    action.execute(this)
}
