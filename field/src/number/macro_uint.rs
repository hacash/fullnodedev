

#[allow(unused)]
macro_rules! from_uint {
    ($class:ident, $vn:ident, $vt:ty, $tt:ty) => (
        impl From<$tt> for $class {
            fn from(item: $tt) -> Self {
                $class { $vn: item as $vt }
            }
        }
    )
}

macro_rules! from_uint_unsigned {
    ($class:ident, $vn:ident, $vt:ty, $tt:ty) => (
        impl From<$tt> for $class {
            fn from(item: $tt) -> Self {
                let v = item as $vt;
                if v > $class::MAX {
                    panic!("{} overflow: {} > {}", stringify!($class), v, $class::MAX)
                }
                $class { $vn: v }
            }
        }
    )
}

macro_rules! from_uint_unsigned_u128 {
    ($class:ident, $vn:ident, $vt:ty) => (
        impl From<u128> for $class {
            fn from(item: u128) -> Self {
                if item > $class::MAX as u128 {
                    panic!("{} overflow: {} > {}", stringify!($class), item, $class::MAX)
                }
                $class { $vn: item as $vt }
            }
        }
    )
}

macro_rules! from_uint_signed {
    ($class:ident, $vn:ident, $vt:ty, $tt:ty) => (
        impl From<$tt> for $class {
            fn from(item: $tt) -> Self {
                if item < 0 {
                    panic!("{} cannot be negative", stringify!($class))
                }
                let v = item as $vt;
                if v > $class::MAX {
                    panic!("{} overflow: {} > {}", stringify!($class), v, $class::MAX)
                }
                $class { $vn: v }
            }
        }
    )
}

macro_rules! from_uint_signed_i128 {
    ($class:ident, $vn:ident, $vt:ty) => (
        impl From<i128> for $class {
            fn from(item: i128) -> Self {
                if item < 0 {
                    panic!("{} cannot be negative", stringify!($class))
                }
                if item > $class::MAX as i128 {
                    panic!("{} overflow: {} > {}", stringify!($class), item, $class::MAX)
                }
                $class { $vn: item as $vt }
            }
        }
    )
}

#[allow(unused)]
macro_rules! from_uint_ary {
    ($class:ident, $vn:ident, $vt:ty, $( $tt:ty ),+) => (
        $(
            from_uint!{$class, $vn, $vt, $tt}
        )+
    )
}

macro_rules! from_uint_all {
    ($class:ident, $vn:ident, $vt:ty) => (
        from_uint_unsigned!{$class, $vn, $vt, u8}
        from_uint_unsigned!{$class, $vn, $vt, u16}
        from_uint_unsigned!{$class, $vn, $vt, u32}
        from_uint_unsigned!{$class, $vn, $vt, u64}
        from_uint_unsigned_u128!{$class, $vn, $vt}
        from_uint_signed!{$class, $vn, $vt, i8}
        from_uint_signed!{$class, $vn, $vt, i16}
        from_uint_signed!{$class, $vn, $vt, i32}
        from_uint_signed!{$class, $vn, $vt, i64}
        from_uint_signed_i128!{$class, $vn, $vt}
    )
}

