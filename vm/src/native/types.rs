/// Helper: generates the type-specific dispatch method.
macro_rules! native_dispatch_method {
    (func, $EnumName:ident, $ErrCode:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $_tar_uint_tys:expr )+) => {
        pub fn call(hei: u64, idx: u8, v: &[u8]) -> VmrtRes<(Value, i64)> {
            let cty = Self::try_from_u8(idx)?;
            match cty {
                $(
                    Self::$name => $name(hei, v).map(|r| {
                        assert_eq!($rty, r.ty());
                        (r, $gas)
                    }),
                )+
                _ => unreachable!(),
            }
        }
    };
    (ctl, $EnumName:ident, $ErrCode:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $_tar_uint_tys:expr )+) => {};
    (env, $EnumName:ident, $ErrCode:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $_tar_uint_tys:expr )+) => {};
}

macro_rules! native_tar_uint_tys_api {
    (func, $EnumName:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $tar_uint_tys:expr )+) => {
        pub fn tar_uint_tys_of(&self) -> &'static [ValueTy] {
            match self {
                $( Self::$name => $tar_uint_tys, )+
                Self::Null => &[],
            }
        }

        pub fn tar_uint_tys(idx: u8) -> Option<&'static [ValueTy]> {
            Self::try_from_u8(idx).ok().and_then(|n| {
                let tys = n.tar_uint_tys_of();
                if tys.is_empty() {
                    None
                } else {
                    Some(tys)
                }
            })
        }
    };
    (ctl, $EnumName:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $tar_uint_tys:expr )+) => {};
    (env, $EnumName:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $tar_uint_tys:expr )+) => {};
}

/// Unified macro for native func / ctl / env metadata.
/// `func` additionally generates `call()`, while `ctl/env` keep runtime dispatch outside this macro.
/// Optional `tar_uint_tys` (`func` only): compile-time uint width for **numeric literals** at each
/// argv position; variables/expressions are unchanged. Use `&[]` when unchecked.
macro_rules! native_func_env_define {
    ( $kind:ident, $EnumName:ident, $ErrCode:ident,
      $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr, $tar_uint_tys:expr )+ ) => {

#[allow(non_camel_case_types)]
#[repr(u8)]
#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum $EnumName {
    #[default] Null = 0u8,
    $( $name = $v, )+
}

impl $EnumName {
    $(
    concat_idents::concat_idents!{ const_name = idx_, $name {
    #[allow(non_upper_case_globals)]
    pub const const_name: u8 = $v;
    }}
    )+

    #[inline]
    pub fn try_from_u8(idx: u8) -> VmrtRes<Self> {
        match idx {
            $( x if x == Self::$name as u8 => Ok(Self::$name), )+
            _ => itr_err_fmt!($ErrCode, "not find {} idx {}", stringify!($EnumName), idx),
        }
    }

    native_dispatch_method!($kind, $EnumName, $ErrCode, $( $name = $v, $argv_len, $gas, $rty, $tar_uint_tys )+);
    native_tar_uint_tys_api!($kind, $EnumName, $( $name = $v, $argv_len, $gas, $rty, $tar_uint_tys )+);

    pub const fn gas_of(&self) -> i64 {
        match self {
            $( Self::$name => $gas, )+
            Self::Null => 0,
        }
    }

    pub fn gas(idx: u8) -> VmrtRes<i64> {
        Ok(Self::try_from_u8(idx)?.gas_of())
    }

    pub fn name(&self) -> &'static str {
        match self {
            $( Self::$name => stringify!($name), )+
            _ => unreachable!()
        }
    }

    pub fn from_name(name: &str) -> Option<(u8, $EnumName)> {
        Some(match name {
            $( stringify!($name) => (Self::$name as u8, Self::$name), )+
            _ => return None
        })
    }

    pub fn has_idx(idx: u8) -> bool {
        match idx {
            $( $v => true, )+
            _ => false,
        }
    }

    pub fn argv_len(idx: u8) -> Option<usize> {
        match idx {
            $( $v => Some($argv_len), )+
            _ => None,
        }
    }

    pub fn argv_len_of(&self) -> usize {
        match self {
            $( Self::$name => $argv_len, )+
            Self::Null => 0,
        }
    }
}

    };
}
