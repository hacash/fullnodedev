use field::*;

use super::rt::ItrErrCode::*;
use super::rt::*;
use super::value::*;

include!("hash.rs");
include!("signature.rs");
include!("types.rs");
include!("amount.rs");
include!("address.rs");
include!("ascii.rs");
include!("defer.rs");
include!("intent.rs");

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
    verify_signature   = 104, 3,       96,    Bool
    ascii_parse_flat_kv = 120, 2,      64,    Tuple
    ascii_validate_transform = 121, 2, 24,    Tuple
    ascii_u128_dec_unit = 122, 2,      24,    Tuple
    ascii_hex_lower    = 123, 1,       20,    Tuple
    ascii_base58_validate_or_echo = 124, 1, 20, Tuple
}

/*
    Native ctl define (VM runtime control, modify tx-local state)
*/
native_func_env_define! { ctl, NativeCtl, NativeCtlError,
    // idx, argv_len, gas, ValueType
    defer              = 150,   1,        8,    Nil
    intent_new         = 151,   1,        8,    U64
    intent_use         = 152,   1,        8,    Nil
    intent_pop         = 153,   0,        8,    Nil
    intent_put         = 154,   2,       12,    Nil
    intent_get         = 155,   1,       10,    Bytes
    intent_take        = 156,   1,       12,    Bytes
}

/*
    Native env define (VM context reads, stack 0→1)
*/
native_func_env_define! { env, NativeEnv, NativeEnvError,
    // idx, argv_len, gas, ValueType
    context_address    = 1,    0,        6,    Address
    intent_current     = 2,    0,        6,    U64
}

include!("call.rs");
