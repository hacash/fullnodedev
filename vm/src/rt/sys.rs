
macro_rules! std_mem_transmute  {
    ($v: expr) => { 
        unsafe { std::mem::transmute($v) }
    }
}

macro_rules! enum_try_from_u8_by_variant {
    (
        $EnumName:ident,
        $ErrCode:expr,
        $ErrFmt:literal,
        [ $( $Variant:ident ),+ $(,)? ]
    ) => {
        impl $EnumName {
            #[inline]
            pub fn try_from_u8(v: u8) -> VmrtRes<Self> {
                match v {
                    $( x if x == Self::$Variant as u8 => Ok(Self::$Variant), )+
                    _ => itr_err_fmt!($ErrCode, $ErrFmt, v),
                }
            }
        }
    };
}
