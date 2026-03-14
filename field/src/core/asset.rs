
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct AssetAmt {
    pub serial: Fold64,
    pub amount: Fold64,
}

impl Parse for AssetAmt {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut serial = Fold64::new();
        let mut amount = Fold64::new();
        let mut seek = serial.parse(buf)?;
        seek += amount.parse(&buf[seek..])?;
        *self = Self { serial, amount }.checked()?;
        Ok(seek)
    }
}

impl Serialize for AssetAmt {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.serial.serialize_to(out);
        self.amount.serialize_to(out);
    }
    fn size(&self) -> usize {
        self.serial.size() + self.amount.size()
    }
}

impl_field_only_new!{AssetAmt}

impl ToJSON for AssetAmt {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"serial\":{},\"amount\":{}}}",
            self.serial.to_json_fmt(fmt),
            self.amount.to_json_fmt(fmt)
        )
    }
}

impl FromJSON for AssetAmt {
    fn from_json(&mut self, json_str: &str) -> Ret<()> {
        let pairs = json_split_object(json_str)?;
        let mut serial = self.serial;
        let mut amount = self.amount;
        for (k, v) in pairs {
            if k == "serial" {
                serial.from_json(v)?;
            } else if k == "amount" {
                amount.from_json(v)?;
            }
        }
        *self = Self { serial, amount }.checked()?;
        Ok(())
    }
}

impl std::fmt::Display for AssetAmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"{{{}:{}}}", self.serial, self.amount)
    }
}

impl std::cmp::Ord for AssetAmt {
    fn cmp(&self, other: &Self) -> Ordering {
        assert!(*self.serial == *other.serial);
        self.amount.cmp(&other.amount)
    }
}

impl std::cmp::PartialOrd for AssetAmt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


macro_rules! checked_opt {
    ($name : ident) => {
        pub fn $name(&self, other: &Self) -> Ret<Self> {
            let err = ||errf!("cannot do {} with asset {} and {}", stringify!($name), self, other);
            if *self.serial != *other.serial {
                return err()
            }
            match (*self.amount).$name(*other.amount) {
                Some(v) => Self{
                    serial: self.serial,
                    amount: Fold64::from(v)?,
                }.checked(),
                _ => err(),
            }
        }
        
    };
}

impl AssetAmt {

    checked_opt!{ checked_add }
    checked_opt!{ checked_sub }

    pub fn checked(self) -> Ret<Self> {
        if *self.serial == 0 {
            return errf!("AssetAmt.serial cannot be zero")
        }
        Ok(Self{
            serial: self.serial.checked()?,
            amount: self.amount.checked()?,
        })
    }

}



impl AssetAmt {

    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_serial(serial: Fold64) -> Ret<Self> {
        Self {
            serial,
            ..Default::default()
        }.checked()
    }

    pub fn from(s: u64, a: u64) -> Ret<Self> {
        Self {
            serial: Fold64::from(s)?,
            amount: Fold64::from(a)?,
        }.checked()
    }

}

