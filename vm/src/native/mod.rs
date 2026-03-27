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
    Native env define (VM context reads, stack 0→1)
*/
native_func_env_define! { env, NativeEnv, NativeEnvError,
    // idx, argv_len, gas, ValueType
    context_address    = 1,    0,        6,    Address
}

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
    defer              = 1,     1,        8,    Nil
    // intent
    intent_new         = 21,    1,       32,    Handle
    intent_use         = 22,    1,        8,    Nil
    intent_pop         = 23,    0,        8,    Nil
    intent_exists      = 24,    1,       10,    Bool
    intent_kind        = 25,    0,        8,    Bytes
    intent_kind_is     = 26,    1,        8,    Bool
    intent_destroy     = 27,    0,       10,    Nil
    intent_destroy_if_empty = 28, 0,     10,    Bool
    intent_clear       = 29,    0,       10,    Nil
    intent_len         = 30,    0,       10,    U64
    intent_has         = 31,    1,       10,    Bool
    intent_keys        = 32,    0,       16,    Compo
    intent_keys_page   = 33,    2,       16,    Tuple
    intent_keys_from   = 34,    2,       16,    Tuple
    intent_get         = 35,    1,       10,    Bytes
    intent_get_or      = 36,    2,       12,    Bytes
    intent_require     = 37,    1,       10,    Bytes
    intent_require_eq  = 38,    2,       10,    Bytes
    intent_require_absent = 39, 1,       10,    Nil
    intent_require_many = 40,   1,       16,    Compo
    intent_require_map = 41,    1,       16,    Compo
    intent_has_all     = 42,    1,       12,    Bool
    intent_has_any     = 43,    1,       12,    Bool
    intent_put         = 44,    2,       24,    Nil
    intent_put_if_absent = 45,  2,       24,    Bool
    intent_put_if_absent_or_match = 46, 2,  24, Bool
    intent_put_pairs   = 47,    1,       32,    Nil
    intent_replace     = 48,    2,       14,    Bytes
    intent_replace_if  = 49,    3,       16,    Bool
    intent_move        = 50,    2,       14,    Nil
    intent_take        = 51,    1,       12,    Bytes
    intent_take_or     = 52,    2,       14,    Bytes
    intent_take_if     = 53,    2,       14,    Tuple
    intent_take_many   = 54,    1,       16,    Compo
    intent_take_map    = 55,    1,       16,    Compo
    intent_consume     = 56,    1,       14,    Bytes
    intent_consume_many = 57,   1,       16,    Compo
    intent_del         = 58,    1,       10,    Nil
    intent_del_if      = 59,    2,       14,    Bool
    intent_del_many    = 60,    1,       12,    U64
    intent_append      = 61,    2,       14,    U64
    intent_inc         = 62,    2,       14,    U64
    intent_add         = 63,    2,       14,    U64
    intent_sub         = 64,    2,       14,    U64
}


include!("call.rs");
