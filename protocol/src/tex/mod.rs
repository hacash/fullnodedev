use std::any::*;

use dyn_clone::*;

use basis::component::*;
use basis::interface::*;
use basis::method::*;
use field::*;
use sys::*;

use super::action::*;
use super::operate::*;
use super::setup::*;
use super::state::*;
use super::*;

static SETTLEMENT_ADDR: Address = ADDRESS_ONEX;

#[inline]
fn tex_check_settlement_addr_privakey() -> Rerr {
    if !SETTLEMENT_ADDR.is_privakey() {
        return errf!(
            "tex settlement address {} must be PRIVAKEY type",
            SETTLEMENT_ADDR
        );
    }
    Ok(())
}

#[inline]
fn tex_hac_amount_must_whole_zhu(amt: &Amount) -> Ret<u64> {
    if amt.is_zero() {
        return Ok(0);
    }
    let zhu = amt.to_zhu_u128()?;
    if zhu == 0 {
        return errf!("tex HAC balance must be zero or at least 1 zhu");
    }
    if zhu > u64::MAX as u128 {
        return errf!("tex HAC balance zhu overflow");
    }
    let zhu = zhu as u64;
    if amt.cmp(&Amount::zhu(zhu)) != std::cmp::Ordering::Equal {
        return errf!("tex HAC balance must be an exact whole-zhu amount");
    }
    Ok(zhu)
}

#[inline]
fn tex_check_addr_whole_zhu(ctx: &mut dyn Context, addr: &Address) -> Rerr {
    let bls = CoreState::wrap(ctx.state())
        .balance(addr)
        .unwrap_or_default();
    tex_hac_amount_must_whole_zhu(&bls.hacash).map(|_| ())
}

fn tex_check_asset_serial(ctx: &mut dyn Context, serial: Fold64) -> Rerr {
    if serial.is_zero() {
        return errf!("tex asset serial cannot be zero");
    }
    {
        let tex = ctx.tex_ledger();
        if tex.asset_checked.contains(&serial) {
            return Ok(());
        }
    }
    let exist = {
        let state = CoreState::wrap(ctx.state());
        state.asset(&serial).is_some()
    };
    if !exist {
        return errf!("tex asset <{}> does not exist", serial.uint());
    }
    ctx.tex_ledger().asset_checked.insert(serial);
    Ok(())
}

include! {"interface.rs"}
include! {"transfer.rs"}
include! {"condition.rs"}
include! {"settle.rs"}
include! {"cell.rs"}
include! {"action.rs"}

/*
    action register
*/
action_register! {

    TexCellAct   // 35

}
