



macro_rules! fixed_define_body {
    ($class:ident, $size: expr) => {

        concat_idents!{ fixed_zero = ZERO_, $class {
        #[allow(non_upper_case_globals)]
        static fixed_zero: OnceLock<$class> = OnceLock::new();
        }}

        #[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
        pub struct $class {
            bytes: [u8; $size],
        }

        impl Default for $class {
            fn default() -> Self {
                $class {
                    bytes: [0u8; $size],
                }
            }
        }

        impl Display for $class {
            fn fmt(&self,f: &mut Formatter) -> Result {
                write!(f,"{}", hex::encode(&self.bytes))
            }
        }

        impl Index<usize> for $class {
            type Output = u8;
            fn index(&self, idx: usize) -> &Self::Output {
                &self.bytes[idx]
            }
        }

        impl IndexMut<usize> for $class {
            fn index_mut(&mut self, idx: usize) -> &mut Self::Output{
                &mut self.bytes[idx]
            }
        }

        impl Deref for $class {
            type Target = [u8; $size];
            fn deref(&self) -> &[u8; $size] {
                &self.bytes
            }
        }

        impl AsRef<[u8]> for $class {
            fn as_ref(&self) -> &[u8] {
                self.bytes.as_slice()
            }
        }
        
        impl AsMut<[u8]> for $class {
            fn as_mut(&mut self) -> &mut [u8] {
                &mut self.bytes
            }
        }

        impl $class {
            pub fn is_zero(&self) -> bool { self.bytes.iter().all(|&x| x == 0) }
            pub fn not_zero(&self) -> bool { !self.is_zero() }
            pub fn to_vec(&self) -> Vec<u8> { self.bytes.to_vec() }
            pub fn must_vec(v: Vec<u8>) -> Self { Self::must(&v) }
        }

        impl Parse for $class {
            fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
                let bts = bufeat_ref(buf, $size)?;
                self.bytes.copy_from_slice(bts);
                Ok($size)
            }
        }

        impl Serialize for $class {
            fn serialize_to(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.bytes);
            }
            fn size(&self) -> usize {
                $size
            }
        }


        impl_field_only_new!{$class}

        impl ToJSON for $class {
            fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
                if $size == 1 {
                    return self.bytes[0].to_string();
                }
                let body = match fmt.binary {
                    JSONBinaryFormat::Hex => format!("0x{}", hex::encode(&self.bytes)),
                    JSONBinaryFormat::Base58Check => {
                        if $size == 21 {
                            let version = self.bytes[0];
                            let data = &self.bytes[1..];
                            format!("b58:{}", data.to_base58check(version))
                        } else {
                            format!("0x{}", hex::encode(&self.bytes))
                        }
                    },
                    JSONBinaryFormat::Base64 => format!("b64:{}", BASE64_STANDARD.encode(&self.bytes)),
                };
                format!("\"{}\"", body)
            }
        }

        impl FromJSON for $class {
            fn from_json(&mut self, json: &str) -> Ret<()> {
                if $size == 1 {
                    let s = json_expect_unquoted(json)?;
                    if let Ok(v) = s.parse::<u8>() {
                        self.bytes[0] = v; return Ok(());
                    }
                    return errf!("cannot parse {} from: {}", stringify!($class), s);
                }
                // generic binary
                let data = json_decode_binary(json)?;
                if data.len() != $size {
                    return errf!("{} size error, need {}, but got {}", stringify!($class), $size, data.len());
                }
                self.bytes.copy_from_slice(&data);
                Ok(())
            }
        }

        impl Hex for $class {
            fn from_hex(buf: &[u8]) -> Ret<Self> {
                let bts = bytes_from_hex(buf, $size)?;
                Ok(Self {
                    bytes: bts.try_into().unwrap()
                })
            }
            fn to_hex(&self) -> String {
                hex::encode(&self.bytes)
            }
        }

        impl Base64 for $class {
            fn to_base64(&self) -> String {
                BASE64_STANDARD.encode(self)
            }
        }

        impl Readable for $class {
            fn from_readable(v: &[u8]) -> Ret<Self> {
                Ok(Self {
                    bytes: bytes_from_readable_string(v, $size).try_into().unwrap()
                })
            }
            fn to_readable(&self) -> String {
                bytes_to_readable_string(&self.bytes)
            }
            fn to_readable_left(&self) -> String {
               self.to_readable().trim().to_owned()
            }
            fn to_readable_or_hex(&self) -> String {
                maybe!(check_readable_string(&self.bytes), self.to_readable(), self.to_hex())
            }
        }


        impl $class {

            pub const SIZE: usize = $size as usize;
            pub const DEFAULT: Self = Self{ bytes: [0u8; $size] };

            pub fn zero_ref() -> &'static Self {
                concat_idents!{ fixed_zero = ZERO_, $class {
                fixed_zero.get_or_init(||Self::DEFAULT)
                }}
            }

            pub const fn from(v: [u8; $size]) -> Self {
                Self {
                    bytes: v
                }
            }

            pub fn into_array(self) -> [u8; $size] {
                self.bytes
            }

            pub fn to_array(self) -> [u8; $size] {
                self.bytes
            }

            pub fn as_array(&self) -> &[u8; $size] {
                &self.bytes
            }

            pub fn into_vec(self) -> Vec<u8> {
                self.bytes.into()
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.bytes
            }

        }


    }
}

macro_rules! fixed_define {
    ($class:ident, $size: expr) => {
        fixed_define_body!{$class, $size}
    }
}



fixed_define!{Fixed1,  1}
fixed_define!{Fixed2,  2}
fixed_define!{Fixed3,  3}
fixed_define!{Fixed4,  4}
fixed_define!{Fixed5,  5}
fixed_define!{Fixed6,  6}
fixed_define!{Fixed7,  7}
fixed_define!{Fixed8,  8}
fixed_define!{Fixed9,  9}
fixed_define!{Fixed10, 10}
fixed_define!{Fixed12, 12}
fixed_define!{Fixed15, 15}
fixed_define!{Fixed16, 16}
fixed_define!{Fixed18, 18}
fixed_define!{Fixed20, 20}
fixed_define!{Fixed21, 21}
fixed_define!{Fixed32, 32}
fixed_define!{Fixed33, 33}
fixed_define!{Fixed64, 64}


/*
* Bool
*/
pub type Bool = Fixed1;

impl Bool {

    pub fn check(&self) -> bool {
        self[0] != 0
    }

    pub fn new(v: bool) -> Self where Self: Sized {
        Self {
            bytes: [ maybe!(v, 1, 0)]
        }
    }

}
