#[macro_export]
macro_rules! action_define {
    ($class:ident, $kid:expr, $scope:expr, $mintxty:expr, $extra9:expr, $reqsign:expr,
        { $( $item:ident : $ty:ty )* },
        ($dself:ident, $desc:expr),
        ($pself:ident, $pctx:ident, $pgas:ident $exec:expr)
    ) => {

        #[derive(Default, Debug, Clone, PartialEq, Eq)]
        pub struct $class {
            kind: Uint2,
            $(
                pub $item: $ty,
            )*
        }


        impl Parse for $class {
            fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
                let mut mv;
                mv = self.kind.parse(&buf)?;
                $(
                    mv += self.$item.parse(&buf[mv..])?;
                )*
                Ok(mv)
            }
        }

        impl Serialize for $class {
            fn serialize(&self) -> Vec<u8> {
                vec![
                    self.kind.serialize(),
                    $(
                        self.$item.serialize()
                    ),*
                ].concat()
            }
            fn size(&self) -> usize {
                [
                    self.kind.size(),
                    $(
                        self.$item.size()
                    ),*
                ].iter().sum()
            }
        }


        impl Field for $class {
            fn new() -> Self {
                Self {
                    kind: Uint2::from(Self::KIND),
                    ..Default::default()
                }
            }
        }

        impl ToJSON for $class {
            fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
                let mut res = String::from("{");
                res.push_str(&format!("\"kind\":{}", self.kind.to_json_fmt(fmt)));
                $(
                    res.push(',');
                    res.push_str(&format!("\"{}\":{}", stringify!($item), self.$item.to_json_fmt(fmt)));
                )*
                res.push('}');
                res
            }
        }

        impl FromJSON for $class {
            fn from_json(&mut self, json_str: &str) -> Ret<()> {
                let pairs = json_split_object(json_str)?;
                for (k, v) in pairs {
                    if k == "kind" {
                        self.kind.from_json(v)?;
                    }
                    $(
                        if k == stringify!($item) {
                            self.$item.from_json(v)?;
                        }
                    )*
                }
                if *self.kind != Self::KIND {
                    return errf!(
                        "action kind mismatch: expected {} but got {}",
                        Self::KIND,
                        *self.kind
                    )
                }
                Ok(())
            }
        }

        impl ActExec for $class {
            fn execute(&$pself, $pctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
                use std::any::Any;
                $crate::upgrade::check_gated_action($pctx.env().block.height, $pself.kind())
                    .into_xret()?;
                $crate::action::check_action_tx_type($pctx.env().tx.ty, $pself).into_xret()?;
                if !$pctx.env().chain.fast_sync {
                    check_action_scope($pctx.exec_from(), $pself).into_xret()?;
                }
                #[allow(unused_mut)]
                let mut $pgas: u32 = $pself.size() as u32;
                let res: Ret<Vec<u8>> = (|| -> Ret<Vec<u8>> { $exec })();
                let res = res.into_xret()?;
                do_action_hook($pself.kind(), $pself as &dyn Any, $pctx).into_xret()?;
                Ok(($pgas, res))
            }
        }

        impl Description for $class {
            fn to_description(&$dself) -> String { $desc }
        }

        impl Action for $class {
            fn kind(&self) -> u16 { *self.kind }
            fn scope(&self) -> ActScope { $scope }
            fn min_tx_type(&self) -> u8 { $mintxty }
            fn extra9(&$pself) -> bool { $extra9 }
            fn req_sign(&$pself) -> Vec<AddrOrPtr> { $reqsign.to_vec() }
            fn as_any(&self) -> &dyn Any { self }
        }

        impl $class {
            pub const KIND: u16 = $kid;
            pub const IDX: u8   = ($kid % 256) as u8;

            pub fn downcast(a: &Box<dyn Action>) -> Option<&Self> {
                let a: &dyn Any = a.as_ref().as_any();
                a.downcast_ref::<Self>()
            }

        }


    };

    ($class:ident, $kid:expr, $scope:expr, $mintxty:expr, $extra9:expr, $reqsign:expr,
        { $( $item:ident : $ty:ty )* },
        ($dself:ident, $desc:expr),
        ($pself:ident, $pctx:ident, $pgas:ident $exec:expr)
    ) => {
        action_define!{
            $class, $kid, $scope, $mintxty, $extra9, $reqsign,
            { $( $item : $ty )* },
            ($dself, $desc),
            ($pself, $pctx, $pgas $exec)
        }
    };

    ($class:ident, $kid:expr, $scope:expr, $mintxty:expr, $extra9:expr, $reqsign:expr,
        { $( $item:ident : $ty:ty )* },
        ($pself:ident, $pctx:ident, $pgas:ident $exec:expr)
    ) => {
        action_define!{
            $class, $kid, $scope, $mintxty, $extra9, $reqsign, { $( $item : $ty )* },
            (self, "".to_owned()),
            ($pself, $pctx, $pgas $exec)
        }
    };
}

#[macro_export]
macro_rules! action_register {
    ( @vm $( $kty:ident )+ ) => {
        pub const ACTION_CODEC_KINDS: &'static [u16] = &[
            $(<$kty>::KIND,)+
        ];
        pub fn try_create(kind: u16, buf: &[u8]) -> Ret<Option<(Box<dyn Action>, usize)>> {
            match kind {
                $(<$kty>::KIND => {
                    let (act, sk) = <$kty>::create(buf)?;
                    Ok(Some((Box::new(act), sk)))
                },)+
                _ => Ok(None)
            }
        }
        pub fn try_json_decode(kind: u16, json: &str) -> Ret<Option<Box<dyn Action>>> {
            match kind {
                $(<$kty>::KIND => {
                    let mut act = <$kty>::new();
                    act.from_json(json)?;
                    Ok(Some(Box::new(act)))
                },)+
                _ => Ok(None)
            }
        }
        pub fn register(builder: SetupBuilder) -> SetupBuilder {
            builder.register_codec(ACTION_CODEC_KINDS, try_create, try_json_decode, true)
        }
    };
    ( $( $kty:ident )+ ) => {
        pub const ACTION_CODEC_KINDS: &'static [u16] = &[
            $(<$kty>::KIND,)+
        ];
        pub fn try_create(kind: u16, buf: &[u8]) -> Ret<Option<(Box<dyn Action>, usize)>> {
            match kind {
                $(<$kty>::KIND => {
                    let (act, sk) = <$kty>::create(buf)?;
                    Ok(Some((Box::new(act), sk)))
                },)+
                _ => Ok(None)
            }
        }
        pub fn try_json_decode(kind: u16, json: &str) -> Ret<Option<Box<dyn Action>>> {
            match kind {
                $(<$kty>::KIND => {
                    let mut act = <$kty>::new();
                    act.from_json(json)?;
                    Ok(Some(Box::new(act)))
                },)+
                _ => Ok(None)
            }
        }
        pub fn register(builder: SetupBuilder) -> SetupBuilder {
            builder.register_codec(ACTION_CODEC_KINDS, try_create, try_json_decode, false)
        }
    };
}

//////////////////// TEST  ////////////////////

// test define action
action_define! { Test63856464969364, 9527,
    ActScope::CALL, // scope
    1,
    false,
    [],
    {
        id: Uint1
        addr: Address
    },
    (self, "Test action".to_owned()),
    (self, _ctx, gas {
        errf!("never call")
        // Ok(vec![])
    })
}
