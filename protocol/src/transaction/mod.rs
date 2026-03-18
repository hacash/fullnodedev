use std::collections::*;

use basis::component::*;
use basis::interface::*;
use field::*;
use sys::*;

use super::action::*;
use super::context::*;
use super::operate;
use super::state::*;
use super::*;

include! {"util.rs"}
include! {"macro.rs"}
include! {"type3.rs"}
include! {"coinbase.rs"}
include! {"create.rs"}
include! {"store.rs"}

/*
* define
*/
transaction_define_legacy! { TransactionType1, 1u8 }
transaction_define_legacy! { TransactionType2, 2u8 }

/*
* register
*/
transaction_register! {
    TransactionCoinbase
    TransactionType1
    TransactionType2
    TransactionType3
}
