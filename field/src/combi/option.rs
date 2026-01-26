

/* 
* create macro
*/
#[macro_export] 
macro_rules! combi_option {
    ($class:ident, $t1:ty, $t2:ty) => (

        #[derive(Clone, PartialEq, Eq)]
        pub enum $class {
            Val1($t1),
            Val2($t2),
        }

        impl std::fmt::Debug for $class {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f,"[option]")
            }
        }

        impl Default for $class {
            fn default() -> Self { Self::Val1(<$t1>::default()) }
        }


        impl Parse for $class {

            fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
                let swt = Uint1::build(buf)?;
                let buf = &buf[1..];
                Ok(1 + maybe!(*swt == 0, {
                    let (v, sk) = <$t1>::create(buf)?;
                    *self = Self::Val1( v );
                    sk
                }, {
                    let (v, sk) = <$t2>::create(buf)?;
                    *self = Self::Val2( v );
                    sk
                }))
            }
        }

        impl Serialize for $class {

            fn serialize_to(&self, out: &mut Vec<u8>) {
                match self {
                    Self::Val1(v1) => {
                        out.push(0u8);
                        v1.serialize_to(out);
                    },
                    Self::Val2(v2) => {
                        out.push(1u8);
                        v2.serialize_to(out);
                    },
                };
            }

            fn size(&self) -> usize {
                1 + match self {
                    Self::Val1(v1) => v1.size(),
                    Self::Val2(v2) => v2.size(),
                }
            }

        }

        impl_field_only_new!{$class}


        impl $class {
            

        }




    )
}
