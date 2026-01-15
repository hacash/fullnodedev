

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(i8)]
pub enum ActLv {
    TopOnly       =  -4i8, // only this single one on top
    TopUnique     =  -3,      // top and unique
    Top           =  -2,      // must on top
    Ast           =  -1,      // on act cond AST 
    MainCall      =   0,      // must in tx main call with depth 0
    ContractCall  =   1,      // abst call or other contract call
    Any           = 127,      // any where
}

impl From<ActLv> for i8 {
    fn from(n: ActLv) -> i8 {
        n as i8
    }
}

impl From<&ActLv> for i8 {
    fn from(n: &ActLv) -> i8 {
        (*n).clone() as i8
    }
}


impl ActLv {
    
    pub fn check_depth(&self, cd: &CallDepth) -> Rerr {
        let al: i8 = self.into();
        mayerr!( al < cd.0, errf!("Action level {} not support be called in depth {}", al, cd.0))
    }

}




/**************************************************/





#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct CallDepth(i8);

impl From<CallDepth> for i8 {
    fn from(n: CallDepth) -> i8 {
        n.0 as i8
    }
}

impl From<&CallDepth> for i8 {
    fn from(n: &CallDepth) -> i8 {
        n.0 as i8
    }
}

// impl PartialEq for CallDepth {
//     fn eq(&self, other: &Self) -> bool {
//         self.0 == other.0
//     }
// }

impl CallDepth {
    
    pub fn new(d: i8) -> Self {
        Self(d)
    }

    pub fn forward(&mut self) {
        self.0 += 1;
    }

    pub fn back(&mut self) {
        self.0 -= 1;
    }

    pub fn to_isize(&self) -> i8 {
        self.0
    }

}
