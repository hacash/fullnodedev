

// create macro
#[macro_export] 
macro_rules! combi_optional {
    ($class:ident, $item:ident : $vty:ty) => (


        #[derive(Default, Clone, PartialEq, Eq)]
        pub struct $class {
            exist: Bool,
            $item: Option<$vty>,
        }

        impl std::fmt::Debug for $class {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f,"[ifval]")
            }
        }

        impl Parse for $class {

            fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
                let mut seek = self.exist.parse_from(buf) ?;
                if self.is_exist() {
                    let mut val = <$vty>::new();
                    seek += val.parse_from(buf) ?;
                    self.$item = Some(val);
                }
                Ok(seek)
            }
        }

        impl Serialize for $class {

            fn serialize_to(&self, out: &mut Vec<u8>) {
                self.exist.serialize_to(out);
                if self.is_exist() {
                    self.$item.as_ref().unwrap().serialize_to(out);
                }
            }

            fn size(&self) -> usize {
                let mut size = self.exist.size();
                if self.is_exist() {
                    size += self.$item.as_ref().unwrap().size();
                }
                size
            }

        }

        impl_field_only_new!{$class}

        impl ToJSON for $class {
            fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
                if self.is_exist() {
                    self.$item.as_ref().unwrap().to_json_fmt(fmt)
                } else {
                    "null".to_string()
                }
            }
        }

        impl FromJSON for $class {
            fn from_json(&mut self, json_str: &str) -> Ret<()> {
                let raw = json_str.trim();
                if raw == "null" {
                    self.exist = Bool::new(false);
                    self.$item = None;
                } else if raw.is_empty() {
                    return errf!("json value empty");
                } else {
                    self.exist = Bool::new(true);
                    let mut val = <$vty>::new();
                    val.from_json(raw)?; // Pass original if it contains nested JSON
                    self.$item = Some(val);
                }
                Ok(())
            }
        }

        impl $class {
            
            pub fn is_exist(&self) -> bool {
                self.exist.check()
            }

            pub fn must(v: $vty) -> $class {
                $class {
                    exist: Bool::new(true),
                    $item: Some(v),
                }
            }

            pub fn from_value(ifv: Option<$vty>) -> $class {
                match ifv {
                    Some(v) => <$class>::must(v),
                    _ => <$class>::default(),
                }
            }

            pub fn if_value(&self) -> Option<& $vty> {
                match &self.$item {
                    Some(v) => Some(&v),
                    None => None,
                }
            }
            
            // clone
            pub fn value(&self) -> $vty {
                maybe!(self.exist.check(), self.$item.as_ref().unwrap().clone(), <$vty>::default())
            }
            

        }




    )
}
