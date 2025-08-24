use std::collections::*;
use std::fmt::{Debug, Display, Formatter, Result};
use std::cmp::Ordering;
use std::ops::{
    Deref, Index, IndexMut, Add, Sub, Mul, Div, 
    AddAssign, SubAssign, MulAssign, DivAssign
};

use concat_idents::concat_idents;
use base64::prelude::*;
use dyn_clone::*;

// use num_bigint::BigInt;
// use num_bigint::Sign::{Minus, Plus};
// use num_traits::{FromPrimitive, ToPrimitive, Num};

use sys::*;


pub mod interface;


include!{"ini.rs"}
include!{"util.rs"}
include!{"impl.rs"}
include!{"empty.rs"}

// number
include!{"number/macro_compute.rs"}
include!{"number/macro_uint.rs"}
include!{"number/uint.rs"}
include!{"number/fold64.rs"}

// bytes
include!{"bytes/fixed.rs"}
include!{"bytes/datas.rs"}

// combi
include!{"combi/struct.rs"}
include!{"combi/list.rs"}
include!{"combi/optional.rs"}
include!{"combi/option.rs"}
include!{"combi/revenum.rs"}
include!{"combi/dynlist.rs"}
include!{"combi/dynvec.rs"}

// core
include!{"core/define.rs"}
include!{"core/address.rs"}
include!{"core/amount.rs"}
include!{"core/diamond.rs"}
include!{"core/diamond_smelt.rs"}
include!{"core/status.rs"}
include!{"core/asset.rs"}

// component
include!{"component/sign.rs"}
include!{"component/asset.rs"}
include!{"component/balance.rs"}
include!{"component/total.rs"}
include!{"component/channel.rs"}





