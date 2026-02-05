


macro_rules! ord_impl {
    ($class:ident, $vn: ident) => (   
        impl PartialOrd for $class {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for $class {
            #[inline]
            fn cmp(&self, other: &Self) -> Ordering {
                self.$vn.cmp(&other.$vn)
            }
        }
    )   
}
    

#[allow(unused)]
macro_rules! compute_opt_impl {
    ($class:ident, $vn: ident, $op: ident, $opt: ident ) => (   
        impl $op for $class {
            type Output = Self;
            #[inline]
            fn $opt(self, other: Self) -> Self {
                Self {$vn: self.$vn.$opt(other.$vn)}
            }
        }
    )
}

macro_rules! compute_opt_impl_checked {
    ($class:ident, $vn: ident, $op: ident, $opt: ident, $checked_opt: ident) => (   
        impl $op for $class {
            type Output = Self;
            #[inline]
            fn $opt(self, other: Self) -> Self {
                let v = self.$vn.$checked_opt(other.$vn)
                    .expect(concat!(stringify!($class), " ", stringify!($opt), " overflow"));
                Self {$vn: v}.checked()
                    .expect(concat!(stringify!($class), " ", stringify!($opt), " overflow max"))
            }
        }
    )
}

macro_rules! compute_opt_impl_checked_div {
    ($class:ident, $vn: ident) => (   
        impl std::ops::Div for $class {
            type Output = Self;
            #[inline]
            fn div(self, other: Self) -> Self {
                if other.$vn == 0 {
                    panic!("{} div divide by zero", stringify!($class))
                }
                let v = self.$vn.checked_div(other.$vn)
                    .expect(concat!(stringify!($class), " div overflow"));
                Self {$vn: v}.checked()
                    .expect(concat!(stringify!($class), " div overflow max"))
            }
        }
    )
}


#[allow(unused)]
macro_rules! compute_other_impl {
    ($class:ident, $vn: ident, $op: ident, $opt: ident, $vty:ty, $eqty:ty) => (   
        impl $op<$eqty> for $class {
            type Output = Self;
            #[inline]
            fn $opt(self, other: $eqty) -> Self {
                Self {$vn: self.$vn.$opt(other as $vty)}
            }
        }
    )
}

macro_rules! compute_other_impl_checked {
    ($class:ident, $vn: ident, $op: ident, $opt: ident, $checked_opt: ident, $vty:ty, $eqty:ty) => (   
        impl $op<$eqty> for $class {
            type Output = Self;
            #[inline]
            fn $opt(self, other: $eqty) -> Self {
                let v = self.$vn.$checked_opt(other as $vty)
                    .expect(concat!(stringify!($class), " ", stringify!($opt), " overflow"));
                Self {$vn: v}.checked()
                    .expect(concat!(stringify!($class), " ", stringify!($opt), " overflow max"))
            }
        }
    )
}

macro_rules! compute_other_impl_checked_div {
    ($class:ident, $vn: ident, $vty:ty, $eqty:ty) => (   
        impl std::ops::Div<$eqty> for $class {
            type Output = Self;
            #[inline]
            fn div(self, other: $eqty) -> Self {
                let r = other as $vty;
                if r == 0 {
                    panic!("{} div divide by zero", stringify!($class))
                }
                let v = self.$vn.checked_div(r)
                    .expect(concat!(stringify!($class), " div overflow"));
                Self {$vn: v}.checked()
                    .expect(concat!(stringify!($class), " div overflow max"))
            }
        }
    )
}


#[allow(unused)]
macro_rules! compute_assign_impl {
    ($class:ident, $vn:ident, $op:ident, $opt: ident) => (
        concat_idents!{opa = $op, Assign {
            impl opa for $class {
                concat_idents!{opa2 = $opt, _assign {
                    #[inline]
                    fn opa2(&mut self, other: Self) {
                        self.$vn.opa2(other.$vn);
                    }
                }}
            }   
        }}

    )
}

macro_rules! compute_assign_impl_checked {
    ($class:ident, $vn:ident, $op:ident, $opt: ident) => (
        concat_idents!{opa = $op, Assign {
            impl opa for $class {
                concat_idents!{opa2 = $opt, _assign {
                    #[inline]
                    fn opa2(&mut self, other: Self) {
                        *self = <Self as std::ops::$op>::$opt(*self, other);
                    }
                }}
            }   
        }}

    )
}


#[allow(unused)]
macro_rules! compute_assign_other_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty, $eqty:ty) => (
        concat_idents!{opa = $op, Assign {
            impl opa<$eqty> for $class {
                concat_idents!{opa2 = $opt, _assign {
                    #[inline]
                    fn opa2(&mut self, other: $eqty) {
                        self.$vn.opa2(other as $vty);
                    }
                }}
            }
        }}
    )
}

macro_rules! compute_assign_other_impl_checked {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty, $eqty:ty) => (
        concat_idents!{opa = $op, Assign {
            impl opa<$eqty> for $class {
                concat_idents!{opa2 = $opt, _assign {
                    #[inline]
                    fn opa2(&mut self, other: $eqty) {
                        *self = <Self as std::ops::$op<$eqty>>::$opt(*self, other);
                    }
                }}
            }
        }}
    )
}


#[allow(unused)]
macro_rules! compute_other_list_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty, $( $eqtys:ty ),+) => (   
        $(
            compute_other_impl!{$class, $vn, $op, $opt, $vty, $eqtys}
        )+
    )
}

macro_rules! compute_other_list_impl_checked {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $checked_opt:ident, $vty:ty, $( $eqtys:ty ),+) => (   
        $(
            compute_other_impl_checked!{$class, $vn, $op, $opt, $checked_opt, $vty, $eqtys}
        )+
    )
}

macro_rules! compute_other_list_impl_checked_div {
    ($class:ident, $vn:ident, $vty:ty, $( $eqtys:ty ),+) => (   
        $(
            compute_other_impl_checked_div!{$class, $vn, $vty, $eqtys}
        )+
    )
}


#[allow(unused)]
macro_rules! compute_other_all_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty) => ( 
        compute_other_list_impl!{$class, $vn, $op, $opt, $vty, 
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        } 
    )
}


#[allow(unused)]
macro_rules! compute_assign_other_list_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty, $( $eqtys:ty ),+) => (   
        $(
            compute_assign_other_impl!{$class, $vn, $op, $opt, $vty, $eqtys}
        )+
    )
}

macro_rules! compute_assign_other_list_impl_checked {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty, $( $eqtys:ty ),+) => (   
        $(
            compute_assign_other_impl_checked!{$class, $vn, $op, $opt, $vty, $eqtys}
        )+
    )
}


#[allow(unused)]
macro_rules! compute_assign_other_all_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty) => ( 
        compute_assign_other_list_impl!{$class, $vn, $op, $opt, $vty, 
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        } 
    )
}


#[allow(unused)]
macro_rules! compute_one_impl {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $vty:ty) => ( 
        compute_opt_impl!{$class, $vn, $op, $opt}
        compute_other_all_impl!{$class, $vn, $op, $opt, $vty}
        compute_assign_impl!{$class, $vn, $op, $opt}
        compute_assign_other_all_impl!{$class, $vn, $op, $opt, $vty}
    )
}


#[allow(unused)]
macro_rules! compute_impl {
    ($class:ident, $vn:ident, $vty:ty) => ( 
        compute_one_impl!{$class,$vn, Add, add, $vty}
        compute_one_impl!{$class,$vn, Sub, sub, $vty}
        compute_one_impl!{$class,$vn, Mul, mul, $vty}
        compute_one_impl!{$class,$vn, Div, div, $vty}
    )
}

macro_rules! compute_one_impl_checked {
    ($class:ident, $vn:ident, $op:ident, $opt:ident, $checked_opt:ident, $vty:ty) => ( 
        compute_opt_impl_checked!{$class, $vn, $op, $opt, $checked_opt}
        compute_other_list_impl_checked!{$class, $vn, $op, $opt, $checked_opt, $vty, 
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        }
        compute_assign_impl_checked!{$class, $vn, $op, $opt}
        compute_assign_other_list_impl_checked!{$class, $vn, $op, $opt, $vty, 
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        }
    )
}

macro_rules! compute_impl_checked {
    ($class:ident, $vn:ident, $vty:ty) => ( 
        compute_one_impl_checked!{$class,$vn, Add, add, checked_add, $vty}
        compute_one_impl_checked!{$class,$vn, Sub, sub, checked_sub, $vty}
        compute_one_impl_checked!{$class,$vn, Mul, mul, checked_mul, $vty}
        compute_opt_impl_checked_div!{$class, $vn}
        compute_other_list_impl_checked_div!{$class, $vn, $vty,
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        }
        compute_assign_impl_checked!{$class, $vn, Div, div}
        compute_assign_other_list_impl_checked!{$class, $vn, Div, div, $vty, 
            i8, u8, i16, u16, i32, u32, i64, u64, i128, u128
        }
    )
}