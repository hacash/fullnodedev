

/* 
* create macro
* combi_revenum: Val2 wire format = t2_value (for t2 whose value is already >= swtv, e.g. Addrptr 20+index)
* combi_revenum_old: Val2 wire format = t2_value + swtv (for t2 whose raw value can be 0..swtv-1, e.g. AddressW1 count)
*/
#[macro_export]
macro_rules! combi_revenum {
    ($class:ident, $t1:ty, $t2:ty, $swtv: expr) => (
        #[derive(Clone, PartialEq, Eq)]
        pub enum $class {
            Val1($t1),
            Val2($t2),
        }
        impl std::fmt::Debug for $class {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f,"[enum]")
            }
        }
        impl Default for $class {
            fn default() -> Self { Self::Val1(<$t1>::default()) }
        }
        impl Parse for $class {
            fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
                if buf.len() < 1 {
                    return Err("buf too short".to_owned())
                }
                if buf[0] < $swtv {
                    let (v, sk) = <$t1>::create(buf)?;
                    *self = Self::Val1(v);
                    Ok(sk)
                } else {
                    let prefix = [buf[0]];
                    let (v, sk) = <$t2 as $crate::ParsePrefix>::create_with_prefix(&prefix, &buf[1..])?;
                    *self = Self::Val2(v);
                    Ok(sk)
                }
            }
        }
        impl Serialize for $class {
            fn serialize_to(&self, out: &mut Vec<u8>) {
                match self {
                    Self::Val1(v1) => v1.serialize_to(out),
                    Self::Val2(v2) => v2.serialize_to(out),
                }
            }
            fn size(&self) -> usize {
                match self {
                    Self::Val1(v1) => v1.size(),
                    Self::Val2(v2) => v2.size(),
                }
            }
        }
        impl_field_only_new!{$class}
        impl ToJSON for $class {
            fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
                match self {
                    Self::Val1(v) => format!("{{\"type\":1,\"value\":{}}}", v.to_json_fmt(fmt)),
                    Self::Val2(v) => format!("{{\"type\":2,\"value\":{}}}", v.to_json_fmt(fmt)),
                }
            }
        }
        impl FromJSON for $class {
            fn from_json(&mut self, json: &str) -> Ret<()> {
                let pairs = json_split_object(json);
                let mut ty = 0u8;
                let mut val_str = "";
                for (k, v) in pairs {
                    if k == "type" { ty = v.parse::<u8>().map_err(|e: std::num::ParseIntError| e.to_string())?; }
                    else if k == "value" { val_str = v; }
                }
                if ty == 1 {
                    let mut v = <$t1>::new();
                    v.from_json(val_str)?;
                    *self = Self::Val1(v);
                } else if ty == 2 {
                    let mut v = <$t2>::new();
                    v.from_json(val_str)?;
                    *self = Self::Val2(v);
                } else {
                    return errf!("invalid revenum type: {}", ty);
                }
                Ok(())
            }
        }
        impl $class {}
    )
}

#[macro_export] 
macro_rules! combi_revenum_old {
    ($class:ident, $t1:ty, $t2:ty, $swtv: expr) => (

        #[derive(Clone, PartialEq, Eq)]
        pub enum $class {
            Val1($t1),
            Val2($t2),
        }

        impl std::fmt::Debug for $class {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f,"[enum]")
            }
        }

        impl Default for $class {
            fn default() -> Self { Self::Val1(<$t1>::default()) }
        }


        impl Parse for $class {

            fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
                if buf.len() < 1 {
                    return Err("buf too short".to_owned())
                }
                if buf[0] < $swtv {
                    let (v, sk) = <$t1>::create(buf)?;
                    *self = Self::Val1( v );
                    Ok(sk)
                }else{
                    let first = buf[0].checked_sub($swtv)
                        .ok_or_else(|| "revenum first byte invalid".to_owned())?;
                    let prefix = [first];
                    let (v, sk) = <$t2 as $crate::ParsePrefix>::create_with_prefix(&prefix, &buf[1..])?;
                    *self = Self::Val2( v );
                    Ok(sk)
                }
            }
        }

        impl Serialize for $class {

            fn serialize_to(&self, out: &mut Vec<u8>) {
                match self {
                    Self::Val1(v1) => v1.serialize_to(out),
                    Self::Val2(v2) => {
                        let start = out.len();
                        v2.serialize_to(out);
                        let mxv = out[start] as usize + $swtv as usize;
                        if mxv > 255 {
                            panic!("mark value too big")
                        }
                        out[start] = mxv as u8;
                    },
                }
            }

            fn size(&self) -> usize {
                match self {
                    Self::Val1(v1) => v1.size(),
                    Self::Val2(v2) => v2.size(),
                }
            }

        }

        impl_field_only_new!{$class}

        impl ToJSON for $class {
            fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
                match self {
                    Self::Val1(v) => format!("{{\"type\":1,\"value\":{}}}", v.to_json_fmt(fmt)),
                    Self::Val2(v) => format!("{{\"type\":2,\"value\":{}}}", v.to_json_fmt(fmt)),
                }
            }
        }

        impl FromJSON for $class {
            fn from_json(&mut self, json: &str) -> Ret<()> {
                let pairs = json_split_object(json);
                let mut ty = 0u8;
                let mut val_str = "";
                for (k, v) in pairs {
                    if k == "type" { ty = v.parse::<u8>().map_err(|e: std::num::ParseIntError| e.to_string())?; }
                    else if k == "value" { val_str = v; }
                }
                if ty == 1 {
                    let mut v = <$t1>::new();
                    v.from_json(val_str)?;
                    *self = Self::Val1(v);
                } else if ty == 2 {
                    let mut v = <$t2>::new();
                    v.from_json(val_str)?;
                    *self = Self::Val2(v);
                } else {
                    return errf!("invalid revenum type: {}", ty);
                }
                Ok(())
            }
        }


        impl $class {
            

        }




    )
}
