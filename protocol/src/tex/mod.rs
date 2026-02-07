use std::any::*;

use dyn_clone::*;


use sys::*;
use field::*;
use basis::component::*;
use basis::interface::*;
use basis::method::*;

use super::*;
use super::state::*;
use super::setup::*;
use super::operate::*;
use super::action::*;


static SETTLEMENT_ADDR: Address = ADDRESS_ONEX;


fn tex_check_asset_serial(ctx: &mut dyn Context, serial: Fold64) -> Rerr {
    if serial.is_zero() {
        return errf!("tex asset serial cannot be zero")
    }
    {
        let tex = ctx.tex_ledger();
        if tex.asset_checked.contains(&serial) {
            return Ok(())
        }
    }
    let exist = {
        let state = CoreState::wrap(ctx.state());
        state.asset(&serial).is_some()
    };
    if !exist {
        return errf!("tex asset <{}> not exist", serial.uint())
    }
    ctx.tex_ledger().asset_checked.insert(serial);
    Ok(())
}



include!{"interface.rs"}
include!{"transfer.rs"}
include!{"condition.rs"}
include!{"settle.rs"}
include!{"cell.rs"}
include!{"action.rs"}





/*
    action register
*/
action_register! {

    TexCellAct

}
