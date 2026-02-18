
use std::sync::Arc;


/* #[repr(u8)] #[derive(Debug, Clone, Copy)] pub enum CallEntry { Main = 1, Abst = 2, } */





pub const FN_SIGN_WIDTH: usize = 4;
pub type FnSign = [u8; FN_SIGN_WIDTH];

pub fn calc_func_sign(name: &str) -> FnSign {
    Hash::from(sys::sha3(name)).check().into_array()
}

pub fn checked_func_sign(s: &[u8]) -> VmrtRes<FnSign> {
    if s.len() != FN_SIGN_WIDTH {
        return itr_err!(CastParamFail, "fn sign size error")
    }
    Ok(s.to_vec().try_into().unwrap())
}



 
pub trait ToHex { fn to_hex(&self) -> String; }
impl ToHex for [u8; FN_SIGN_WIDTH] {
    fn to_hex(&self) -> String {
        hex::encode(self)
    }
}




//////////////////////////////////////////


#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
pub enum CodeType {
    #[default] Bytecode = 0,
    IRNode      = 1,
}

impl CodeType {
    pub fn parse(n: u8) -> VmrtRes<Self> {
        let ct = n & 0b00000111;
        if ![
            Self::Bytecode as u8,
            Self::IRNode as u8,
        ].contains(&ct) {
            return itr_err_code!(ItrErrCode::CodeTypeError)
        }
        Ok(std_mem_transmute!(ct))
    }
}



//////////////////////////////////////////



pub enum FnKey {
    Abst(AbstCall),
    User(FnSign),
}


#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum FnConf {
    Public = 0b10000000,
}

#[derive(Debug, Clone, Default)]
pub struct ByteView {
    bytes: Arc<[u8]>,
    offset: usize,
    limit: usize,
}

impl ByteView {
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        let bytes: Arc<[u8]> = bytes.into();
        let limit = bytes.len();
        Self { bytes, offset: 0, limit }
    }

    pub fn from_arc(bytes: Arc<[u8]>) -> Self {
        let limit = bytes.len();
        Self { bytes, offset: 0, limit }
    }

    pub fn len(&self) -> usize {
        self.limit - self.offset
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[self.offset..self.limit]
    }

    pub fn slice(&self, offset: usize, limit: usize) -> VmrtRes<Self> {
        if offset > limit || limit > self.len() {
            return itr_err_code!(ItrErrCode::CodeOverflow)
        }
        Ok(Self {
            bytes: self.bytes.clone(),
            offset: self.offset + offset,
            limit: self.offset + limit,
        })
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl AsRef<[u8]> for ByteView {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<Vec<u8>> for ByteView {
    fn from(value: Vec<u8>) -> Self {
        Self::from_vec(value)
    }
}

impl From<Arc<[u8]>> for ByteView {
    fn from(value: Arc<[u8]>) -> Self {
        Self::from_arc(value)
    }
}

#[cfg(test)]
mod call_tests {
    use super::*;

    #[test]
    fn checked_func_sign_uses_cast_param_fail_for_wrong_size() {
        let err = checked_func_sign(&[1, 2, 3]).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastParamFail);
    }
}


#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FnObj {
    pub confs: u8, // binary switch
    pub agvty: Option<FuncArgvTypes>,
    pub ctype: CodeType,
    pub codes: ByteView,
    compiled: Arc<std::sync::OnceLock<ByteView>>,
}

impl FnObj {
    
    pub fn check_conf(&self, cnf: FnConf) -> bool {
        let cnfset = cnf as u8;
        self.confs & cnfset == cnfset
    } 

    pub fn create(mks: u8, codes: Vec<u8>, agvty: Option<FuncArgvTypes>) -> VmrtRes<Self> {
        let ctype = CodeType::parse(mks)?;
        Ok(Self {
            confs: mks & 0b11111000,
            agvty,
            ctype,
            codes: ByteView::from_vec(codes),
            compiled: Arc::new(std::sync::OnceLock::new()),
        })
    }

    pub fn plain(ctype: CodeType, codes: impl Into<ByteView>, confs: u8, agvty: Option<FuncArgvTypes>) -> Self {
        Self {
            confs,
            agvty,
            ctype,
            codes: codes.into(),
            compiled: Arc::new(std::sync::OnceLock::new()),
        }
    }

    pub fn exec_bytecodes(&self, height: u64) -> VmrtRes<ByteView> {
        use CodeType::*;
        Ok(match self.ctype {
            Bytecode => self.codes.clone(),
            IRNode => {
                if let Some(cached) = self.compiled.get() {
                    return Ok(cached.clone());
                }
                let compiled = ByteView::from_vec(runtime_irs_to_exec_bytecodes(self.codes.as_slice(), height)?);
                let _ = self.compiled.set(compiled.clone());
                compiled
            }
        })
    }

    pub fn into_array(self) -> Vec<u8> {
        self.codes.into_vec()
    }
}



#[derive(Debug, Clone)]
pub enum CallExit {
    Abort,          // throw nil
    Throw,          // throw <ERR>
    Finish,         // func ret nil
    Return,         // func ret <DATA>
    Call(Funcptr),  // call func
}




#[derive(Debug, Clone)]
pub enum CallTarget {
    This,
    Self_,
    Super,
    Libidx(u8),
    // Addr(ContractAddress),
}

impl CallTarget {
    pub fn idx(&self) -> u8 {
        match self {
            CallTarget::Libidx(i) => *i,
            _ => 0,
        }
    }
}

/* Entry mode: Main, P2sh, Abst Call  mode: Outer, Inner, View, Pure */
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ExecMode {
    #[default] Main, // tx main call
    P2sh, // p2sh script verify
    Abst, // contract abstract call
    Outer, 
    Inner,
    View,  // read-only (can read state, cannot write)
    Pure,  // no-state (cannot read or write state)
}


#[derive(Debug, Clone)]
pub struct Funcptr {
    pub mode: ExecMode,
    pub is_callcode: bool,
    pub target: CallTarget,
    pub fnsign: FnSign,
}


//////////////////////////////////////////


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
        impl AbstCall {
            pub fn check(n: u8) -> VmrtErr {
                if [$($v),+].contains(&n) {
                    return Ok(())
                }
                itr_err_fmt!(ItrErrCode::AbstTypeError, "AbstCall type {} not find", n)
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

impl AbstCall {

    pub fn uint(&self) -> u8 {
        *self as u8
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
