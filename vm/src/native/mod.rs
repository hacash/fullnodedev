use field::*;

use super::rt::ItrErrCode::*;
use super::rt::*;
use super::value::*;

include!("hash.rs");
include!("types.rs");
include!("amount.rs");
include!("address.rs");

use ValueTy::*;

/*
    Native call define
*/
native_call_define! {  // idx, arg_num, gas,   ValueType
    context_address    = 1,    0,     6,    Address
    /* */
    hac_to_mei         = 31,   1,     8,    U64
    hac_to_zhu         = 32,   1,     8,    U128
    // hac_to_shuo         = 33,   8,    U128
    mei_to_hac         = 35,   1,     8,    Bytes
    zhu_to_hac         = 36,   1,     8,    Bytes
    // shuo_to_suo         = 37,   8,    Bytes
    address_ptr        = 41,   1,     4,    U8
    /* */
    sha2               = 101,  1,    32,    Bytes
    sha3               = 102,  1,    32,    Bytes
    ripemd160          = 103,  1,    20,    Bytes

}
