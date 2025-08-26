
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
compute_impl!{Fold64, value, u64}
from_uint_all!{Fold64, value, u64}


impl Parse for Fold64 {

    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let bt = bufeatone(buf)?;
        let tl = bt >> 5;
        if tl >= 8 { // error
            return Err(s!("Fold64 format error"))
        }
        let mut body = vec![bt & 0b00011111];
        let n = tl as usize;
        if n > 0 {
            let tail = bufeat(&buf[1..], n)?;
            body = [body, tail].concat();
        }
        let bn = body.len();
        if bn < 8 {
            body = [vec![0u8; 8-bn], body].concat();
        }
        self.value = u64::from_be_bytes(body.try_into().unwrap());
        Ok(1 + n)
    }

}


impl Serialize for Fold64 {

    fn serialize(&self) -> Vec<u8> {
        if self.value > Fold64::MAX {
            never!() // fatal error!!!
        }
        let vs = self.size() as u8;
        let head = vec![(vs - 1) << 5];
        let mut data = self.value.to_be_bytes().to_vec();
        let mv = 8 - vs as usize;
        data = data[mv..].to_vec();
        data[0] ^= head[0];
        data
    }

    fn size(&self) -> usize {
        let v = self.value;
        let mut s = 1;
        for k in FOLDU64XLIST {
            if v < k {
                break; // ok
            }else{
                s += 1;
            }
        }
        s
    }

}


impl_field_only_new!{Fold64}


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
    
}





/************************ test ************************/




/*
    cargo test --fold64_tests
*/
#[cfg(test)]
mod fold64_tests {
    use super::*;

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
    
}













