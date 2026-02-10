use std::collections::*;

use sys::*;
use field::*;
use basis::interface::*;
use basis::component::*;

use super::*;
use super::operate;
use super::action::*;
use super::state::*;


include!{"util.rs"}
include!{"macro.rs"}
include!{"coinbase.rs"}
include!{"create.rs"}
include!{"store.rs"}

/*
* define
*/
transaction_define!{ TransactionType1, 1u8 }
transaction_define!{ TransactionType2, 2u8 }
transaction_define!{ TransactionType3, 3u8 }

/*
* register
*/
transaction_register!{
    TransactionCoinbase
    TransactionType1
    TransactionType2
    TransactionType3
}