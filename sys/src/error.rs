pub type TextRet<T> = Result<T, TextError>;
pub type TextRerr = Result<(), TextError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    // Recoverable execution failure (business/runtime control-flow).
    Revert(String),
    // Unrecoverable execution failure (hard stop).
    Fault(String),
}

pub type ExecRet<T> = Result<T, ExecError>;
pub type ExecRerr = Result<(), ExecError>;
pub type XError = ExecError;
pub type XRet<T> = ExecRet<T>;
pub type XRerr = ExecRerr;

// Text-layer aliases (legacy Ret/Rerr are removed).
pub type Ret<T> = TextRet<T>;
pub type Rerr = TextRerr;

pub const REVERT_PREFIX: &str = "[REVERT] ";

pub fn decode_exec_error_from_text(err: TextError) -> ExecError {
    if let Some(msg) = err.strip_prefix(REVERT_PREFIX) {
        ExecError::revert(msg.to_owned())
    } else {
        ExecError::fault(err)
    }
}

pub fn encode_exec_error_to_text(err: ExecError) -> TextError {
    match err {
        ExecError::Revert(msg) => format!("{}{}", REVERT_PREFIX, msg),
        ExecError::Fault(msg) => msg,
    }
}

impl ExecError {
    pub fn revert(msg: impl Into<String>) -> Self {
        Self::Revert(msg.into())
    }

    pub fn fault(msg: impl Into<String>) -> Self {
        Self::Fault(msg.into())
    }

    pub fn is_revert(&self) -> bool {
        matches!(self, Self::Revert(_))
    }

    pub fn is_fault(&self) -> bool {
        matches!(self, Self::Fault(_))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Revert(msg) => msg,
            Self::Fault(msg) => msg,
        }
    }

    pub fn contains(&self, pat: &str) -> bool {
        self.as_str().contains(pat)
    }
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Revert(msg) => write!(f, "{}{}", REVERT_PREFIX, msg),
            Self::Fault(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<TextError> for ExecError {
    fn from(v: TextError) -> Self {
        decode_exec_error_from_text(v)
    }
}

impl From<&str> for ExecError {
    fn from(v: &str) -> Self {
        Self::fault(v.to_owned())
    }
}

impl From<ExecError> for TextError {
    fn from(v: ExecError) -> Self {
        encode_exec_error_to_text(v)
    }
}

pub trait IntoExecRet<T> {
    fn into_exec_ret(self) -> ExecRet<T>;
}

impl<T> IntoExecRet<T> for TextRet<T> {
    fn into_exec_ret(self) -> ExecRet<T> {
        self.map_err(decode_exec_error_from_text)
    }
}

impl<T> IntoExecRet<T> for ExecRet<T> {
    fn into_exec_ret(self) -> ExecRet<T> {
        self
    }
}

pub trait IntoTextRet<T> {
    fn into_text_ret(self) -> TextRet<T>;
}

impl<T> IntoTextRet<T> for ExecRet<T> {
    fn into_text_ret(self) -> TextRet<T> {
        self.map_err(TextError::from)
    }
}

impl<T> IntoTextRet<T> for TextRet<T> {
    fn into_text_ret(self) -> TextRet<T> {
        self
    }
}

pub trait IntoXRet<T> {
    fn into_xret(self) -> XRet<T>;
}

impl<T> IntoXRet<T> for Ret<T> {
    fn into_xret(self) -> XRet<T> {
        self.into_exec_ret()
    }
}

impl<T> IntoXRet<T> for XRet<T> {
    fn into_xret(self) -> XRet<T> {
        self
    }
}

pub trait IntoTRet<T> {
    fn into_tret(self) -> Ret<T>;
}

impl<T> IntoTRet<T> for XRet<T> {
    fn into_tret(self) -> Ret<T> {
        self.into_text_ret()
    }
}

impl<T> IntoTRet<T> for Ret<T> {
    fn into_tret(self) -> Ret<T> {
        self
    }
}

pub const _BUF_E1: &str = "buffer too short";

#[macro_export]
macro_rules! er {
    ($v:expr) => {
        Some(($v).to_string())
    };
}

#[macro_export]
macro_rules! erf {
    ( $($v:expr),+ ) => { er!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! err {
    ($v:expr) => {
        Err(($v).to_string())
    };
}

#[macro_export]
macro_rules! errf {
    ( $($v:expr),+ ) => { err!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! xerr {
    ($v:expr) => {
        Err($crate::XError::fault(($v).to_string()).into())
    };
}

#[macro_export]
macro_rules! xerr_r {
    ($v:expr) => {
        Err($crate::XError::revert(($v).to_string()).into())
    };
}

#[macro_export]
macro_rules! xerrf {
    ( $($v:expr),+ ) => { xerr!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! xerr_rf {
    ( $($v:expr),+ ) => { xerr_r!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! terrunbox {
    ($errbox:expr) => {
        match $errbox {
            Ok(v) => Ok(v),
            Err(e) => Err(e.to_string()),
        }
    };
}

#[macro_export]
macro_rules! ifter {
    ( $value:expr ) => {
        if let Some(e) = $value {
            return Err(e);
        }
    };
}

#[macro_export]
macro_rules! ifxer {
    ( $value:expr ) => {
        if let Some(e) = $value {
            return Err($crate::XError::fault(e).into());
        }
    };
}

#[macro_export]
macro_rules! xerrunbox {
    ($errbox:expr) => {
        match $errbox {
            Ok(v) => Ok(v),
            Err(e) => Err($crate::XError::fault(e.to_string()).into()),
        }
    };
}

#[macro_export]
macro_rules! err_buf_short {
    () => {
        err!(_BUF_E1)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xerror_into_text_error_uses_revert_prefix_only_for_revert() {
        let u: Error = XError::revert("biz fail").into();
        assert_eq!(u, "[REVERT] biz fail");
        let i: Error = XError::fault("sys fail").into();
        assert_eq!(i, "sys fail");
    }

    #[test]
    fn tret_into_xret_recovers_revert_prefix() {
        let r: Ret<()> = Err("[REVERT] fallback".to_owned());
        let e = r.into_xret().unwrap_err();
        assert!(e.is_revert());
        assert_eq!(e.as_str(), "fallback");
    }

    #[test]
    fn tret_into_xret_without_prefix_is_fault() {
        let r: Ret<()> = Err("hard fail".to_owned());
        let e = r.into_xret().unwrap_err();
        assert!(e.is_fault());
        assert_eq!(e.as_str(), "hard fail");
    }

    #[test]
    fn xerror_display_uses_wire_format() {
        let rec = XError::revert("biz fail").to_string();
        assert_eq!(rec, "[REVERT] biz fail");

        let int = XError::fault("sys fail").to_string();
        assert_eq!(int, "sys fail");
    }

    #[test]
    fn xerr_macros_map_to_exec_error_variants() {
        let u: XRet<()> = xerr_r!("biz fail");
        assert!(u.unwrap_err().is_revert());
        let i: XRet<()> = xerr!("sys fail");
        assert!(i.unwrap_err().is_fault());
    }
}
