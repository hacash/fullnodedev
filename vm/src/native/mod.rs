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
    // idx, gas, ValueType
    hac_to_mei         = 31,   8,    U64
    hac_to_zhu         = 32,   8,    U128
    // hac_to_shuo      = 33,   8,    U128
    mei_to_hac         = 35,   8,    Bytes
    zhu_to_hac         = 36,   8,    Bytes
    // shuo_to_suo      = 37,   8,    Bytes
    address_ptr        = 41,   4,    U8
    /* */
    sha2               = 101, 32,    Bytes
    sha3               = 102, 32,    Bytes
    ripemd160          = 103, 20,    Bytes
}

/*
    Native env define (VM context reads, stack 0→1)
*/
native_func_env_define! { env, NativeEnv, NativeEnvError,
    // idx, gas, ValueType
    context_address    = 1,    6,    Address
}
