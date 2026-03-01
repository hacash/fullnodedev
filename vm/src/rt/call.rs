
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

enum_try_from_u8_by_variant!(
    CodeType,
    ItrErrCode::CodeTypeError,
    "code type {} not find",
    [Bytecode, IRNode]
);

impl CodeType {
    pub const TYPE_MASK: u8 = 0b0000_0011;

    // Parse only the lower 2 bits as code type selector.
    pub fn parse(n: u8) -> VmrtRes<Self> {
        Self::try_from_u8(n & Self::TYPE_MASK)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CodeConf(u8);

impl CodeConf {
    pub const RESERVED_MASK: u8 = 0b1111_1100;

    // Current rule: high 6 bits are reserved and must be zero.
    pub fn parse(raw: u8) -> VmrtRes<Self> {
        if raw & Self::RESERVED_MASK != 0 {
            return itr_err_code!(ItrErrCode::CodeTypeError)
        }
        let _ = CodeType::parse(raw)?;
        Ok(Self(raw))
    }

    pub const fn from_type(code_type: CodeType) -> Self {
        Self(code_type as u8)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }

    pub fn code_type(self) -> CodeType {
        // Safe by construction: CodeConf::parse() already validates type bits.
        match CodeType::try_from_u8(self.0 & CodeType::TYPE_MASK) {
            Ok(v) => v,
            Err(_) => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodePkg {
    pub conf: u8,
    pub data: Vec<u8>,
}

impl CodePkg {
    pub fn code_conf(&self) -> VmrtRes<CodeConf> {
        CodeConf::parse(self.conf)
    }

    pub fn code_type(&self) -> VmrtRes<CodeType> {
        Ok(self.code_conf()?.code_type())
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

    #[test]
    fn codeconf_rejects_reserved_bits() {
        let err = CodeConf::parse(0b1000_0000).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CodeTypeError);
    }

    #[test]
    fn codeconf_rejects_bit2() {
        let err = CodeConf::parse(0b0000_0100).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CodeTypeError);
    }

    #[test]
    fn codeconf_roundtrip_bytecode() {
        let conf = CodeConf::parse(CodeType::Bytecode as u8).unwrap();
        assert_eq!(conf.raw(), CodeType::Bytecode as u8);
        assert!(matches!(conf.code_type(), CodeType::Bytecode));
    }

    #[test]
    fn codepkg_parses_code_type_from_conf() {
        let pkg = CodePkg{
            conf: CodeConf::from_type(CodeType::IRNode).raw(),
            data: vec![],
        };
        assert!(matches!(pkg.code_type().unwrap(), CodeType::IRNode));
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

    pub fn create(fncnf: u8, pkg: CodePkg, agvty: Option<FuncArgvTypes>) -> VmrtRes<Self> {
        let ctype = pkg.code_type()?;
        Ok(Self {
            confs: fncnf,
            agvty,
            ctype,
            codes: ByteView::from_vec(pkg.data),
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
    // Index points to a dispatch-root contract; the actual lookup scope is selected by runtime policy.
    Idx(u8),
    // Addr(ContractAddress),
}

impl CallTarget {
    pub fn root_idx(&self) -> u8 {
        match self {
            CallTarget::Idx(i) => *i,
            _ => 0,
        }
    }
}

/* Entry mode: Main, P2sh, Abst Call  mode: Outer, Inner, View, Pure */
#[repr(u8)]
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

enum_try_from_u8_by_variant!(
    ExecMode,
    ItrErrCode::CallInvalid,
    "exec mode {} not find",
    [Main, P2sh, Abst, Outer, Inner, View, Pure]
);


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
