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
