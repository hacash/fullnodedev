
// Satoshi
pub type SatoshiAuto = Fold64;
combi_optional!{ SatoshiOptional, 
	satoshi: Satoshi 
}
impl SatoshiAuto {
	pub fn to_satoshi(&self) -> Satoshi {
		Satoshi::from( self.uint() )
	}
	pub fn from_satoshi(sat: &Satoshi) -> SatoshiAuto {
		SatoshiAuto::from( sat.uint() ).unwrap()
	}
}


// AddrHac
combi_struct!{ AddrHac,
	address: Address
	amount : Amount
}

// HacAndSat
combi_struct!{ HacSat, 
	amount : Amount
	satoshi: SatoshiOptional
}

// AddrHacSat
combi_struct!{ AddrHacSat, 
	address: Address
	hacsat : HacSat
}

// AddrBalance
combi_struct!{ AddrBalance, 
	address: Address
	balance: Balance
}

combi_list!{ AssetAmtW1, Uint1, AssetAmt }

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    pub hacash: Amount,
    pub satoshi: SatoshiAuto,
    pub diamond: DiamondNumberAuto,
    pub assets: AssetAmtW1,
}

impl Parse for Balance {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut hacash = Amount::new();
        let mut satoshi = SatoshiAuto::new();
        let mut diamond = DiamondNumberAuto::new();
        let mut assets = AssetAmtW1::new();
        let mut seek = hacash.parse(buf)?;
        seek += satoshi.parse(&buf[seek..])?;
        seek += diamond.parse(&buf[seek..])?;
        seek += assets.parse(&buf[seek..])?;
        Self::check_assets(&assets)?;
        self.hacash = hacash;
        self.satoshi = satoshi;
        self.diamond = diamond;
        self.assets = assets;
        Ok(seek)
    }
}

impl Serialize for Balance {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.hacash.serialize_to(out);
        self.satoshi.serialize_to(out);
        self.diamond.serialize_to(out);
        self.assets.serialize_to(out);
    }
    fn size(&self) -> usize {
        self.hacash.size() + self.satoshi.size() + self.diamond.size() + self.assets.size()
    }
}

impl_field_only_new!{Balance}

impl ToJSON for Balance {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"hacash\":{},\"satoshi\":{},\"diamond\":{},\"assets\":{}}}",
            self.hacash.to_json_fmt(fmt),
            self.satoshi.to_json_fmt(fmt),
            self.diamond.to_json_fmt(fmt),
            self.assets.to_json_fmt(fmt)
        )
    }
}

impl FromJSON for Balance {
    fn from_json(&mut self, json_str: &str) -> Ret<()> {
        let pairs = json_split_object(json_str)?;
        let mut hacash = self.hacash.clone();
        let mut satoshi = self.satoshi;
        let mut diamond = self.diamond;
        let mut assets = self.assets.clone();
        for (k, v) in pairs {
            if k == "hacash" {
                hacash.from_json(v)?;
            } else if k == "satoshi" {
                satoshi.from_json(v)?;
            } else if k == "diamond" {
                diamond.from_json(v)?;
            } else if k == "assets" {
                assets.from_json(v)?;
            }
        }
        Self::check_assets(&assets)?;
        self.hacash = hacash;
        self.satoshi = satoshi;
        self.diamond = diamond;
        self.assets = assets;
        Ok(())
    }
}

pub const BALANCE_ASSET_MAX: usize = 20;

impl Balance {
    fn check_assets(assets: &AssetAmtW1) -> Rerr {
        if assets.length() > BALANCE_ASSET_MAX {
            return errf!("balance asset item quantity cannot exceed {}", BALANCE_ASSET_MAX)
        }
        let mut seen = HashSet::new();
        for ast in assets.as_list() {
            ast.clone().checked()?;
            if !seen.insert(ast.serial.uint()) {
                return errf!("balance asset serial {} duplicated", ast.serial)
            }
        }
        Ok(())
    }

	pub fn hac(amt: Amount) -> Self {
		Self {
			hacash: amt,
			..Default::default()
		}
	}

	pub fn asset(&self, seri: Fold64) -> Option<AssetAmt> {
		self.assets.as_list().iter().find(|a|a.serial==seri).map(|a|a.clone())
	}

	pub fn asset_must(&self, seri: Fold64) -> AssetAmt {
		self.asset(seri).unwrap_or(AssetAmt::from_serial(seri).unwrap())
	}

	pub fn asset_set(&mut self, amt: AssetAmt) -> Rerr {
		let amt = if amt.amount.is_zero() { amt } else { amt.checked()? };
		let assets = self.assets.as_mut();
		for i in 0..assets.len() {
			let ast = assets.get_mut(i).unwrap();
			if ast.serial == amt.serial {
				if amt.amount.is_zero() {
					self.assets.drop(i).unwrap();// delete
				} else {
					*ast = amt; // update
				}
				return Ok(())
			}
		}
		if amt.amount.is_zero() {
			return Ok(()) // zero do nothing
		}
		// Fix: Check length before push to avoid inconsistent state
		if self.assets.length() >= BALANCE_ASSET_MAX {
			return errf!("balance asset item quantity cannot exceed {}", BALANCE_ASSET_MAX)
		}
		self.assets.push(amt)?;
		Ok(())
	}


}
