
const BYTEN: u64 = 256;
const FOLDU64SX1: u64 = 32; // 2^^5       // 1byte :                    32
const FOLDU64SX2: u64 = FOLDU64SX1 * BYTEN; // 2byte :                  8192
const FOLDU64SX3: u64 = FOLDU64SX2 * BYTEN; // 3byte :               2097152
const FOLDU64SX4: u64 = FOLDU64SX3 * BYTEN; // 4byte :            5_36870912
const FOLDU64SX5: u64 = FOLDU64SX4 * BYTEN; // 5byte :         1374_38953472
const FOLDU64SX6: u64 = FOLDU64SX5 * BYTEN; // 6byte :       351843_72088832
const FOLDU64SX7: u64 = FOLDU64SX6 * BYTEN; // 7byte :     90071992_54740992
const FOLDU64SX8: u64 = FOLDU64SX7 * BYTEN; // 8byte : 230_58430092_13693952
//                                                  2 30584300 92136939.52

#[allow(unused)]
const FOLDU64XLIST: [u64; 8] = [
    FOLDU64SX1, 
    FOLDU64SX2, 
    FOLDU64SX3, 
    FOLDU64SX4, 
    FOLDU64SX5, 
    FOLDU64SX6, 
    FOLDU64SX7, 
    FOLDU64SX8
];





#[derive(Default, Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub struct Fold64 {
    value: u64,
}


impl Display for Fold64 {
    fn fmt(&self,f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,"{}", self.value)
    }
}

impl Deref for Fold64 {
    type Target = u64;
    fn deref(&self) -> &u64 {
        &self.value
    }
}


ord_impl!{Fold64, value}


impl Parse for Fold64 {

    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let bt = bufeatone(buf)?;
        let n = (bt >> 5) as usize;
        if buf.len() < 1 + n {
            return errf!("Fold64 parse length {} < {}", buf.len(), 1 + n)
        }
        let mut value = (bt & 0b00011111) as u64;
        for i in 0..n {
            value = (value << 8) | buf[1 + i] as u64;
        }
        if value > Fold64::MAX {
            return errf!("Fold64 value {} overflow max {}", value, Fold64::MAX)
        }
        let expect = Fold64{ value }.size();
        if expect != 1 + n {
            return errf!("Fold64 non-canonical size {} expect {}", 1 + n, expect)
        }
        self.value = value;
        Ok(1 + n)
    }

}


impl Serialize for Fold64 {

    fn serialize(&self) -> Vec<u8> {
        if self.value > Fold64::MAX {
            never!() // fatal error!!!
        }
        let vs = self.size() as u8;
        let head = ((vs - 1) << 5) as u8;
        let data = self.value.to_be_bytes();
        let mv = 8 - vs as usize;
        let mut out = Vec::with_capacity(vs as usize);
        out.extend_from_slice(&data[mv..]);
        out[0] = (out[0] & 0b00011111) | head;
        out
    }

    fn size(&self) -> usize {
        let v = self.value;
        if v < FOLDU64SX1 {
            return 1
        }
        let bits = 64 - v.leading_zeros();
        let extra = bits.saturating_sub(5) as usize;
        1 + (extra + 7) / 8
    }

}


impl_field_only_new!{Fold64}

impl ToJSON for Fold64 {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        self.value.to_string()
    }
}

impl FromJSON for Fold64 {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let s = json_expect_unquoted(json)?;
        if let Ok(v) = s.parse::<u64>() {
            self.value = v;
            return Ok(());
        }
        errf!("cannot parse fold64 from: {}", s)
    }
}


impl Fold64 {

    pub const MAX: u64 = FOLDU64SX8 - 1;

    pub const fn max() -> Self {
        Self{ value: Self::MAX }
    }

    pub fn is_zero(&self) -> bool {
        self.value == 0
    }

    pub fn not_zero(&self) -> bool {
        self.value != 0
    }

    pub fn checked(self) -> Ret<Self> {
        if self.value > Self::MAX {
            return errf!("Fold64 value {} cannot more than max {}", self.value, Self::MAX)
        }
        Ok(self)
    }

    pub fn from(v: u64) -> Ret<Self> {
        Self{ value: v }.checked()
    }

    pub fn uint(&self) -> u64 {
        self.value
    }

    pub fn checked_add(self, other: Self) -> Ret<Self> {
        let Some(v) = self.value.checked_add(other.value) else {
            return errf!("Fold64 checked_add overflow")
        };
        Self::from(v)
    }

    pub fn checked_sub(self, other: Self) -> Ret<Self> {
        let Some(v) = self.value.checked_sub(other.value) else {
            return errf!("Fold64 checked_sub overflow")
        };
        Self::from(v)
    }

    pub fn checked_mul(self, other: Self) -> Ret<Self> {
        let Some(v) = self.value.checked_mul(other.value) else {
            return errf!("Fold64 checked_mul overflow")
        };
        Self::from(v)
    }

    pub fn checked_div(self, other: Self) -> Ret<Self> {
        if other.value == 0 {
            return errf!("Fold64 checked_div divide by zero")
        }
        let Some(v) = self.value.checked_div(other.value) else {
            return errf!("Fold64 checked_div overflow")
        };
        Self::from(v)
    }
    
}

impl TryFrom<u64> for Fold64 {
    type Error = String;
    fn try_from(v: u64) -> Ret<Self> {
        Self::from(v)
    }
}

impl TryFrom<u128> for Fold64 {
    type Error = String;
    fn try_from(v: u128) -> Ret<Self> {
        if v > u64::MAX as u128 {
            return errf!("Fold64 value {} overflow u64", v)
        }
        Self::from(v as u64)
    }
}

impl TryFrom<i64> for Fold64 {
    type Error = String;
    fn try_from(v: i64) -> Ret<Self> {
        if v < 0 {
            return errf!("Fold64 cannot be negative {}", v)
        }
        Self::from(v as u64)
    }
}

impl std::ops::Add for Fold64 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        self.checked_add(other).expect("Fold64 add overflow")
    }
}

impl std::ops::Sub for Fold64 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self.checked_sub(other).expect("Fold64 sub overflow")
    }
}

impl std::ops::Mul for Fold64 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        self.checked_mul(other).expect("Fold64 mul overflow")
    }
}

impl std::ops::Div for Fold64 {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        self.checked_div(other).expect("Fold64 div overflow")
    }
}

impl std::ops::AddAssign for Fold64 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::SubAssign for Fold64 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl std::ops::MulAssign for Fold64 {
    fn mul_assign(&mut self, other: Self) {
        *self = *self * other;
    }
}

impl std::ops::DivAssign for Fold64 {
    fn div_assign(&mut self, other: Self) {
        *self = *self / other;
    }
}





/************************ test ************************/




/*
    cargo test --fold64_tests
*/
#[cfg(test)]
mod fold64_tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    /* 
    #[test]
    fn test1() {
        for i in 0..=Fold64::MAX {
            do_t_one(i);
        }
    }
    */

    #[test]
    fn test3() {
        do_t_one(0);
        do_t_one(1);
        do_t_one(2);
        do_t_one(3);
        for lx in FOLDU64XLIST.iter() {
            do_t_one(lx - 3);
            do_t_one(lx - 2);
            do_t_one(lx - 1);
            if *lx < FOLDU64SX8 {
                do_t_one(lx - 0);
                do_t_one(lx + 1);
                do_t_one(lx + 2);
                do_t_one(lx + 3);
            }
        }


    }


    #[test]
    fn test2() {
        do_t_one(0);
        do_t_one(1);
        do_t_one(2);

        do_t_one(                    30);
        do_t_one(                    31);
        do_t_one(                    32);
        do_t_one(                    33);
        do_t_one(                    34);

        do_t_one(                  8190);
        do_t_one(                  8191);
        do_t_one(                  8192);
        do_t_one(                  8193);
        do_t_one(                  8194);
        
        do_t_one(               2097151);
        do_t_one(               2097152);
        do_t_one(               2097153);
        
        do_t_one(            5_36870911);
        do_t_one(            5_36870912);
        do_t_one(            5_36870913);

        do_t_one(         1374_38953471);
        do_t_one(         1374_38953472);
        do_t_one(         1374_38953473);
        
        do_t_one(       351843_72088831);
        do_t_one(       351843_72088832);
        do_t_one(       351843_72088833);
        
        do_t_one(     90071992_54740991);
        do_t_one(     90071992_54740992);
        do_t_one(     90071992_54740993);
        
        do_t_one( 230_58430092_13693950);
        do_t_one( 230_58430092_13693951); // MAX

        // do_t_one( 230_58430992_13693952); // overflow error
        // do_t_one( 230_58430992_13693953); // overflow error

    }

    #[test]
    fn test_boundary_serialization_roundtrip() {
        let cases = [
            (0u64, 1usize),
            (31u64, 1usize),
            (FOLDU64SX1, 2usize),
            (FOLDU64SX2 - 1, 2usize),
            (FOLDU64SX2, 3usize),
            (FOLDU64SX3 - 1, 3usize),
            (FOLDU64SX4, 5usize),
            (FOLDU64SX5 - 1, 5usize),
            (FOLDU64SX5, 6usize),
            (FOLDU64SX6 - 1, 6usize),
            (FOLDU64SX6, 7usize),
            (FOLDU64SX7 - 1, 7usize),
            (Fold64::MAX, 8usize),
        ];
        for (v, size) in cases {
            let fu = Fold64::from(v).unwrap();
            assert_eq!(fu.size(), size);
            let serialized = fu.serialize();
            let head_bits = (serialized[0] >> 5) & 0b111;
            assert_eq!(head_bits as usize, size - 1);
            let mut decoded = Fold64::default();
            decoded.parse(&serialized).unwrap();
            assert_eq!(decoded, fu);
            assert_eq!(decoded.uint(), v);
        }
    }

    #[test]
    fn test_mid_range_roundtrip() {
        let cases = [
            (FOLDU64SX3 + 1234, 4usize),
            (FOLDU64SX4 + 0x1234, 5usize),
            (FOLDU64SX5 + 0x12345, 6usize),
            (FOLDU64SX6 + 0x1234, 7usize),
        ];
        for (value, size) in cases {
            let fu = Fold64::from(value).unwrap();
            assert_eq!(fu.size(), size);
            let serialized = fu.serialize();
            let mut decoded = Fold64::default();
            decoded.parse(&serialized).unwrap();
            assert_eq!(decoded, fu);
            assert_eq!(decoded.uint(), value);
        }
    }

    #[test]
    fn test_parse_rejects_non_canonical() {
        // value 1 encoded with size=2 should be rejected as non-canonical
        let buf = vec![0b0010_0000, 0x01];
        let mut parsed = Fold64::default();
        assert!(parsed.parse(&buf).is_err());
    }

    #[test]
    fn test_checked_arithmetic_errors() {
        let max = Fold64::from(Fold64::MAX).unwrap();
        let one = Fold64::from(1).unwrap();
        assert!(max.checked_add(one).is_err());
        assert!(one.checked_sub(max).is_err());
        assert!(max.checked_mul(Fold64::from(2).unwrap()).is_err());
        assert!(one.checked_div(Fold64::from(0).unwrap()).is_err());
    }

    #[test]
    fn test_try_from_limits() {
        assert!(Fold64::try_from(-1i64).is_err());
        let big = (u64::MAX as u128) + 1;
        assert!(Fold64::try_from(big).is_err());
    }

    #[test]
    #[ignore]
    fn bench_fold64_vs_leb128() {
        // Run with: cargo test -p field fold64_tests::bench_fold64_vs_leb128 -- --ignored --nocapture
        let samples = [
            0u64,
            1u64,
            31u64,
            32u64,
            8191u64,
            8192u64,
            2_097_151u64,
            2_097_152u64,
            5_368_709_11u64,
            5_368_709_12u64,
            1_374_389_534_71u64,
            1_374_389_534_72u64,
            351_843_720_888_31u64,
            351_843_720_888_32u64,
            90_071_992_547_409_91u64,
            90_071_992_547_409_92u64,
            Fold64::MAX,
        ];

        let mut fold_bytes = Vec::with_capacity(samples.len());
        let mut leb_bytes = Vec::with_capacity(samples.len());
        for v in samples.iter() {
            fold_bytes.push(Fold64::from(*v).unwrap().serialize());
            leb_bytes.push(leb128_encode_u64(*v));
        }

        let loops = 200_000usize;

        let now = Instant::now();
        let mut acc = 0u64;
        for i in 0..loops {
            let idx = i % samples.len();
            let mut f = Fold64::default();
            let len = f.parse(black_box(&fold_bytes[idx])).unwrap();
            acc = acc.wrapping_add(f.uint() ^ (len as u64));
        }
        let fold_decode = now.elapsed();

        let now = Instant::now();
        let mut acc2 = 0u64;
        for i in 0..loops {
            let idx = i % samples.len();
            let (v, len) = leb128_decode_u64(black_box(&leb_bytes[idx])).unwrap();
            acc2 = acc2.wrapping_add(v ^ (len as u64));
        }
        let leb_decode = now.elapsed();

        let now = Instant::now();
        let mut acc3 = 0usize;
        for i in 0..loops {
            let idx = i % samples.len();
            let data = Fold64::from(samples[idx]).unwrap().serialize();
            acc3 ^= data.len();
        }
        let fold_encode = now.elapsed();

        let now = Instant::now();
        let mut acc4 = 0usize;
        for i in 0..loops {
            let idx = i % samples.len();
            let data = leb128_encode_u64(samples[idx]);
            acc4 ^= data.len();
        }
        let leb_encode = now.elapsed();

        println!(
            "fold64 decode {:?} encode {:?} (acc {} {})",
            fold_decode, fold_encode, acc, acc3
        );
        println!(
            "leb128 decode {:?} encode {:?} (acc {} {})",
            leb_decode, leb_encode, acc2, acc4
        );
    }

    fn do_t_one(n: u64) {
        let fu = Fold64::from(n).unwrap();
        let mut fu2 = Fold64::from(0).unwrap();
        let _ = fu2.parse(&fu.serialize());
        assert_eq!(fu, fu2);
        assert_eq!(n, fu.uint());
        assert_eq!(n, fu2.uint());
        assert_eq!(fu.serialize(), fu2.serialize());
        println!("{} {} {}", n, fu.serialize().to_hex(), fu2.size())
    }
    
    fn leb128_encode_u64(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                break;
            } else {
                out.push(byte | 0x80);
            }
        }
        out
    }

    fn leb128_decode_u64(buf: &[u8]) -> Ret<(u64, usize)> {
        let mut result = 0u64;
        let mut shift = 0u32;
        for (i, b) in buf.iter().enumerate() {
            let byte = *b as u64;
            result |= (byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok((result, i + 1))
            }
            shift += 7;
            if shift >= 64 {
                return errf!("leb128 overflow")
            }
        }
        errf!("leb128 unterminated")
    }
}



