pub type Ret<T> = Result<T, Error>;
pub type Rerr   = Result<(), Error>;

pub type BRet<T> = Result<T, BError>;
pub type BRerr   = Result<(), BError>;
pub const RECOVERABLE_PREFIX: &str = "[RECOVERABLE] ";
pub const UNRECOVERABLE_PREFIX: &str = "[UNRECOVERABLE] ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BError {
    Recoverable(String),
    Unrecoverable(String),
}

impl BError {
    pub fn recoverable(msg: impl Into<String>) -> Self {
        Self::Recoverable(msg.into())
    }

    pub fn unrecoverable(msg: impl Into<String>) -> Self {
        Self::Unrecoverable(msg.into())
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Recoverable(_))
    }

    pub fn is_unrecoverable(&self) -> bool {
        matches!(self, Self::Unrecoverable(_))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Recoverable(msg) => msg,
            Self::Unrecoverable(msg) => msg,
        }
    }
}

impl std::fmt::Display for BError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recoverable(msg) => write!(f, "Recoverable: {}", msg),
            Self::Unrecoverable(msg) => write!(f, "Unrecoverable: {}", msg),
        }
    }
}

impl std::error::Error for BError {}

impl From<String> for BError {
    fn from(v: String) -> Self {
        Self::Unrecoverable(v)
    }
}

impl From<&str> for BError {
    fn from(v: &str) -> Self {
        Self::Unrecoverable(v.to_owned())
    }
}

impl From<BError> for Error {
    fn from(v: BError) -> Self {
        v.to_string()
    }
}

pub trait IntoBRet<T> {
    fn into_bret(self) -> BRet<T>;
}

impl<T> IntoBRet<T> for Ret<T> {
    fn into_bret(self) -> BRet<T> {
        self.map_err(|e| {
            if let Some(msg) = e.strip_prefix(RECOVERABLE_PREFIX) {
                BError::recoverable(msg.to_owned())
            } else
            if let Some(msg) = e.strip_prefix(UNRECOVERABLE_PREFIX) {
                BError::unrecoverable(msg.to_owned())
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
    ($v:expr) => { Some(($v).to_string()) };
}

#[macro_export]
macro_rules! erf {
    ( $($v:expr),+ ) => { er!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! err {
    ($v:expr) => { Err(($v).to_string()) };
}

#[macro_export]
macro_rules! berr {
    ($v:expr) => { Err($crate::BError::unrecoverable(($v).to_string())) };
}

#[macro_export]
macro_rules! berru {
    ($v:expr) => { Err($crate::BError::recoverable(($v).to_string())) };
}

#[macro_export]
macro_rules! errf {
    ( $($v:expr),+ ) => { err!(format!( $($v),+ )) };
}

#[macro_export]
macro_rules! erru {
    ($v:expr) => { err!(format!("{}{}", $crate::RECOVERABLE_PREFIX, ($v).to_string())) };
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
    return Err(e)
}    
    };
}

#[macro_export]
macro_rules! ifber {
    ( $value:expr ) => {
if let Some(e) = $value {
    return Err($crate::BError::unrecoverable(e))
}
    };
}

#[macro_export]
macro_rules! err_buf_short {
    () => { err!(_BUF_E1) };
}
