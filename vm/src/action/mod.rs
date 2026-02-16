use std::any::*;

use basis::component::*;
use basis::interface::*;
use field::*;
use protocol::action::*;
use protocol::setup::*;
use protocol::state::*;
use protocol::*;
use sys::*;

use super::rt::*;
use super::*;
// use super::space::*;
// use super::util::*;

//

// pub fn try_create(_kind: u16, _buf: &[u8]) -> Ret<Option<(Box<dyn Action>, usize)>> {
//     Ok(None)
// }

include! {"blob.rs"}
include! {"contract.rs"}
include! {"maincall.rs"}
include! {"envfunc.rs"}
include! {"p2sh.rs"}
include! {"p2sh_tool.rs"}

/*
    action register
*/
action_register! {

    TxMessage            // 120
    TxBlob               // 121
    ContractDeploy       // 122
    ContractUpdate       // 123
    ContractMainCall     // 124
    UnlockScriptProve

    EnvHeight
    EnvMainAddr
    EnvCoinbaseAddr

    ViewCheckSign
    ViewBalance
    ViewDiamondInscNum
    ViewDiamondInscGet
}
