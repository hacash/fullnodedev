

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

