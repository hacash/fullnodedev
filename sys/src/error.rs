pub type Ret<T> = Result<T, Error>;
pub type Rerr = Result<(), Error>;

pub type BRet<T> = Result<T, BError>;
pub type BRerr = Result<(), BError>;
pub const UNWIND_PREFIX: &str = "[UNWIND] ";
// Keep compatibility aliases for existing call sites.
pub const RECOVERABLE_PREFIX: &str = UNWIND_PREFIX;
pub const UNRECOVERABLE_PREFIX: &str = "";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BError {
    Unwind(String),
    Interrupt(String),
}

impl BError {
    pub fn unwind(msg: impl Into<String>) -> Self {
        Self::Unwind(msg.into())
    }

    pub fn interrupt(msg: impl Into<String>) -> Self {
        Self::Interrupt(msg.into())
    }

    pub fn recoverable(msg: impl Into<String>) -> Self {
        Self::Unwind(msg.into())
    }

    pub fn unrecoverable(msg: impl Into<String>) -> Self {
        Self::Interrupt(msg.into())
    }

    pub fn is_unwind(&self) -> bool {
        matches!(self, Self::Unwind(_))
    }

    pub fn is_interrupt(&self) -> bool {
        matches!(self, Self::Interrupt(_))
    }

    pub fn is_recoverable(&self) -> bool {
        self.is_unwind()
    }

    pub fn is_unrecoverable(&self) -> bool {
        self.is_interrupt()
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Unwind(msg) => msg,
            Self::Interrupt(msg) => msg,
        }
    }

    pub fn contains(&self, pat: &str) -> bool {
        self.as_str().contains(pat)
    }
}

impl std::fmt::Display for BError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unwind(msg) => write!(f, "{}{}", UNWIND_PREFIX, msg),
            Self::Interrupt(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for BError {}

impl From<String> for BError {
    fn from(v: String) -> Self {
        Self::Interrupt(v)
    }
}

impl From<&str> for BError {
    fn from(v: &str) -> Self {
        Self::Interrupt(v.to_owned())
    }
}

impl From<BError> for Error {
    fn from(v: BError) -> Self {
        match v {
            BError::Unwind(msg) => format!("{}{}", UNWIND_PREFIX, msg),
            BError::Interrupt(msg) => msg,
        }
    }
}

pub trait IntoBRet<T> {
    fn into_bret(self) -> BRet<T>;
}

impl<T> IntoBRet<T> for Ret<T> {
    fn into_bret(self) -> BRet<T> {
        self.map_err(|e| {
            if let Some(msg) = e.strip_prefix(UNWIND_PREFIX) {
                BError::recoverable(msg.to_owned())
            } else {
                BError::unrecoverable(e)
            }
        })
    }
}

impl<T> IntoBRet<T> for BRet<T> {
    fn into_bret(self) -> BRet<T> {
        self
    }
}

pub trait IntoRet<T> {
    fn into_ret(self) -> Ret<T>;
}

impl<T> IntoRet<T> for BRet<T> {
    fn into_ret(self) -> Ret<T> {
        self.map_err(Error::from)
    }
}

impl<T> IntoRet<T> for Ret<T> {
    fn into_ret(self) -> Ret<T> {
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
macro_rules! berr {
    ($v:expr) => {
        Err($crate::BError::unrecoverable(($v).to_string()))
    };
}

#[macro_export]
macro_rules! berru {
    ($v:expr) => {
        Err($crate::BError::recoverable(($v).to_string()))
    };
}

#[macro_export]
macro_rules! errf {
    ( $($v:expr),+ ) => { err!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! erru {
    ($v:expr) => {
        err!(format!("{}{}", $crate::UNWIND_PREFIX, ($v).to_string()))
    };
}

#[macro_export]
macro_rules! erruf {
    ( $($v:expr),+ ) => { erru!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! berrf {
    ( $($v:expr),+ ) => { berr!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! berruf {
    ( $($v:expr),+ ) => { berru!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! errunbox {
    ($errbox:expr) => {
        match $errbox {
            Ok(v) => Ok(v),
            Err(e) => Err(e.to_string()),
        }
    };
}

#[macro_export]
macro_rules! berrunbox {
    ($errbox:expr) => {
        match $errbox {
            Ok(v) => Ok(v),
            Err(e) => Err($crate::BError::unrecoverable(e.to_string())),
        }
    };
}

#[macro_export]
macro_rules! ifer {
    ( $value:expr ) => {
        // Some => Err
        if let Some(e) = $value {
            return Err(e);
        }
    };
}

#[macro_export]
macro_rules! ifber {
    ( $value:expr ) => {
        if let Some(e) = $value {
            return Err($crate::BError::unrecoverable(e));
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
    fn berror_into_error_uses_unwind_prefix_only_for_unwind() {
        let u: Error = BError::recoverable("biz fail").into();
        assert_eq!(u, "[UNWIND] biz fail");
        let i: Error = BError::unrecoverable("sys fail").into();
        assert_eq!(i, "sys fail");
    }

    #[test]
    fn ret_into_bret_recovers_unwind_prefix() {
        let r: Ret<()> = Err("[UNWIND] fallback".to_owned());
        let e = r.into_bret().unwrap_err();
        assert!(e.is_recoverable());
        assert_eq!(e.as_str(), "fallback");
    }

    #[test]
    fn ret_into_bret_without_prefix_is_unrecoverable() {
        let r: Ret<()> = Err("hard fail".to_owned());
        let e = r.into_bret().unwrap_err();
        assert!(e.is_unrecoverable());
        assert_eq!(e.as_str(), "hard fail");
    }

    #[test]
    fn berror_display_uses_wire_format() {
        let rec = BError::recoverable("biz fail").to_string();
        assert_eq!(rec, "[UNWIND] biz fail");

        let int = BError::unrecoverable("sys fail").to_string();
        assert_eq!(int, "sys fail");
    }
}
