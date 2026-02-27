

#[repr(u8)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValueTy {
    #[default]
    Nil         = 0,
    Bool        = 1,
    U8          = 2,
    U16         = 3,
    U32         = 4,
    U64         = 5,
    U128        = 6,
    // U256     = 7 ...      = 8 ...      = 9
    Bytes       = 10,
    Address     = 11,
    // ...      = 12 ...      = 13
    HeapSlice   = 14,
    Compo       = 15
}

pub const RESERVED_U256_TYPE_NAME: &str = "u256";

impl ValueTy {

    pub fn canbe_argv(&self) -> Rerr {
        use ValueTy::*;
        match self {
            Nil | HeapSlice | Compo => errf!("Value Type {:?} cannot be func argv", self),
            _ => Ok(())
        }
    }

    /// Allowed as function return value (Compo/list may only be return type, not parameter)
    pub fn canbe_retval(&self) -> Rerr {
        use ValueTy::*;
        match self {
            Nil | HeapSlice => errf!("Value Type {:?} cannot be func retval", self),
            _ => Ok(())
        }
    }

    pub fn name(&self) -> &'static str {
        // use ValueTy::*;
        match self {
            ValueTy::Nil       => "nil"       ,
            ValueTy::Bool      => "bool"      ,
            ValueTy::U8        => "u8"        ,
            ValueTy::U16       => "u16"       ,
            ValueTy::U32       => "u32"       ,
            ValueTy::U64       => "u64"       ,
            ValueTy::U128      => "u128"      ,
            /* */
            ValueTy::Bytes     => "bytes"     ,
            ValueTy::Address   => "address"   ,
            /* */
            ValueTy::HeapSlice => "heapslice" ,
            ValueTy::Compo     => "compo"     ,
        }
    }

    pub fn is_uint(&self) -> bool {
        matches!(self, ValueTy::U8 | ValueTy::U16 | ValueTy::U32 | ValueTy::U64 | ValueTy::U128)
    }

    pub fn uint_bits(&self) -> Option<u16> {
        match self {
            ValueTy::U8 => Some(8),
            ValueTy::U16 => Some(16),
            ValueTy::U32 => Some(32),
            ValueTy::U64 => Some(64),
            ValueTy::U128 => Some(128),
            _ => None,
        }
    }

    pub fn from_name(s: &str) -> Ret<Self> {
        use ValueTy::*;
        Ok(match s {
            "nil"       => Nil,
            "bool"      => Bool,
            "u8"        => U8,
            "u16"       => U16,
            "u32"       => U32,
            "u64"       => U64,
            "u128"      => U128,
            RESERVED_U256_TYPE_NAME => return errf!("value type '{}' is reserved but not enabled", RESERVED_U256_TYPE_NAME),
            "bytes"     => Bytes,
            "address"   => Address,
            "heapslice" => HeapSlice,
            "compo"     => Compo,
            a => return errf!("not find value type '{}'", a)
        })
    }


    pub fn build(t: u8) -> Ret<Self> {
        use ValueTy::*;
        Ok(match t {
            0  => Nil       ,
            1  => Bool      ,
            2  => U8        ,
            3  => U16       ,
            4  => U32       ,
            5  => U64       ,
            6  => U128      ,
            RESERVED_U256_TYPE_ID => return errf!("ValueTy {} (u256) is reserved but not enabled", RESERVED_U256_TYPE_ID),
            /* */
            10 => Bytes     ,
            11 => Address   ,
            /* */
            14 => HeapSlice ,
            15 => Compo     ,
            _ => return errf!("ValueTy {} not find", t)
        })
    }



}

#[cfg(test)]
mod type_tests {
    use super::*;

    #[test]
    fn reserved_u256_name_and_type_id_are_rejected() {
        assert!(ValueTy::from_name(RESERVED_U256_TYPE_NAME).is_err());
        assert!(ValueTy::build(RESERVED_U256_TYPE_ID).is_err());
    }

    #[test]
    fn uint_helpers_are_consistent_for_active_uints() {
        assert!(ValueTy::U64.is_uint());
        assert_eq!(ValueTy::U64.uint_bits(), Some(64));
        assert!(!ValueTy::Bytes.is_uint());
        assert_eq!(ValueTy::Bytes.uint_bits(), None);
    }
}
