macro_rules! abst_call_type_define {
    ( $( $k:ident : $v:expr , [ $( $atk:ident ),* ] )+ ) => {
        #[allow(non_camel_case_types)]
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
        pub enum AbstCall {
        $(
            $k = $v,
        )+
        }
        enum_try_from_u8_by_variant!(
            AbstCall,
            ItrErrCode::AbstTypeError,
            "AbstCall type {} not find",
            [$( $k ),+]
        );
        impl AbstCall {
            pub fn check(n: u8) -> VmrtErr {
                Self::try_from_u8(n).map(|_| ())
            }
            pub const fn uint(self) -> u8 {
                self as u8
            }
            pub fn from_name(name: &str) -> VmrtRes<Self> {
                Ok(match name {
                    $(
                    stringify!($k) => Self::$k,
                    )+
                    _ => return itr_err_fmt!(ItrErrCode::AbstTypeError, "AbstCall name {} not find", name)
                })
            }
            pub fn param_types(&self) -> Vec<ValueTy> {
                match self {
                    $(
                    Self::$k => vec![ $( ValueTy::$atk ),* ],
                    )+
                }
            }
        }
    }
}

abst_call_type_define! {
    Construct    : 0u8 , [ Bytes ]
    Change       : 1   , [ ]
    Append       : 2   , [ ]

    PermitHAC    : 15  , [ Address, Bytes ]
    PermitSAT    : 16  , [ Address, U64 ]
    PermitHACD   : 17  , [ Address, U32, Bytes ]
    PermitAsset  : 18  , [ Address, U64, U64 ]

    PayableHAC   : 25  , [ Address, Bytes ]
    PayableSAT   : 26  , [ Address, U64 ]
    PayableHACD  : 27  , [ Address, U32, Bytes ]
    PayableAsset : 28  , [ Address, U64, U64 ]
}
