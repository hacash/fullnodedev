







/// Helper: generates the type-specific method (call for func, gas for env).
macro_rules! native_dispatch_method {
    (func, $EnumName:ident, $ErrCode:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr )+) => {
        pub fn call(hei: u64, idx: u8, v: &[u8]) -> VmrtRes<(Value, i64)> {
            let cty: $EnumName = std_mem_transmute!(idx);
            match cty {
                $(
                    Self::$name => $name(hei, v).map(|r| {
                        assert_eq!($rty, r.ty());
                        (r, $gas)
                    }),
                )+
                _ => return itr_err_fmt!($ErrCode, "not find native func idx {}", idx),
            }
        }
    };
    (env, $EnumName:ident, $ErrCode:ident, $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr )+) => {
        pub fn gas(idx: u8) -> VmrtRes<i64> {
            match idx {
                $( $v => Ok($gas), )+
                _ => return itr_err_fmt!($ErrCode, "not find native env idx {}", idx),
            }
        }
    };
}

/// Unified macro for NativeFunc / NativeEnv.
/// Pass `func` for pure functions (generates `call()`), `env` for context reads (generates `gas()`).
macro_rules! native_func_env_define {
    ( $kind:ident, $EnumName:ident, $ErrCode:ident,
      $( $name:ident = $v:expr, $argv_len:expr, $gas:expr, $rty:expr )+ ) => {

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

    native_dispatch_method!($kind, $EnumName, $ErrCode, $( $name = $v, $argv_len, $gas, $rty )+);

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

