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
    Native func define (pure functions, stack 1→1)
*/
native_func_env_define! { func, NativeFunc, NativeFuncError,
    // idx, argv_len, gas, ValueType
    hac_to_mei         = 31,   1,        6,    U64
    hac_to_zhu         = 32,   1,        6,    U128
    u64_to_fold64      = 33,   1,        8,    Bytes
    fold64_to_u64      = 34,   1,        8,    U64
    // hac_to_shuo      = 38,   1,        6,    U128
    pack_asset         = 37,   2,        8,    Bytes
    mei_to_hac         = 35,   1,        6,    Bytes
    zhu_to_hac         = 36,   1,        6,    Bytes
    // shuo_to_suo      = 39,   1,        6,    Bytes
    address_ptr        = 41,   1,        4,    U8
    /* */
    sha2               = 101, 1,       32,    Bytes
    sha3               = 102, 1,       32,    Bytes
    ripemd160          = 103, 1,       20,    Bytes
}

/*
    Native env define (VM context reads, stack 0→1)
*/
native_func_env_define! { env, NativeEnv, NativeEnvError,
    // idx, argv_len, gas, ValueType
    context_address    = 1,    0,        6,    Address
}
