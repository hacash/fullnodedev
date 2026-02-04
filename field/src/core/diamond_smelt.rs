
/*
* Diamond Status
*/
pub const DIAMOND_STATUS_NORMAL                : Uint1 = Uint1::from(1);
pub const DIAMOND_STATUS_LENDING_TO_SYSTEM     : Uint1 = Uint1::from(2);
pub const DIAMOND_STATUS_LENDING_TO_USER       : Uint1 = Uint1::from(3);


/*
* Diamond Inscripts
*/
combi_list!{ Inscripts, 
	Uint1, BytesW1
}

impl Inscripts {
	pub fn array(&self) -> Vec<String> {
		let mut resv = Vec::with_capacity(self.lists.len());
		for li in &self.lists {
			let rdstr = bytes_try_to_readable_string(li.as_ref());
			resv.push(match rdstr {
				None => hex::encode(li.as_ref()),
				Some(s) => s,
			});
		}
		resv
	}
}


/*
* Diamond
*/
combi_struct!{ DiamondSto, 
	status    : Uint1
	address   : Address
	prev_engraved_height : BlockHeight
	inscripts : Inscripts
 }


/*
* DiamondSmelt
*/
combi_struct!{ DiamondSmelt, 
	diamond                   : DiamondName
	number                    : DiamondNumber
	born_height               : BlockHeight
	born_hash                 : Hash // block
	prev_hash                 : Hash // block
	miner_address             : Address
	bid_fee                   : Amount
	nonce                     : Fixed8
	// custom_message           : HashOptional
	average_bid_burn          : Uint2 // unit: mei
	life_gene                 : Hash
}



/*
* DiamondOwnedForm
*/
combi_struct!{ DiamondOwnedForm, 
	names : BytesW4
}
impl DiamondOwnedForm {

	pub fn readable(&self) -> String {
		String::from_utf8_lossy( self.names.as_ref() ).to_string()
	}
	
	pub fn push_one(&mut self, dian: &DiamondName) {
		let mut bytes = dian.serialize();
		self.names.append(&mut bytes).unwrap();
	}

	pub fn drop_one(&mut self, dian: &DiamondName) -> Ret<usize> {
		let list = DiamondNameListMax200::one(dian.clone());
		self.drop(&list)
	}

	pub fn push(&mut self, dian: &DiamondNameListMax200) {
		let mut bytes = dian.form();
		self.names.append(&mut bytes).unwrap();
	}

	// return balance quantity
	pub fn drop(&mut self, dian: &DiamondNameListMax200) -> Ret<usize> {
		const DS: usize = DiamondName::SIZE;
		let mut form = std::mem::take(&mut self.names).into_vec();
		if form.len() % DS != 0 {
			return errf!("DiamondOwnedForm names length {} is not divisible by {}", form.len(), DS)
		}
		let mut len = form.len() / DS;
		let mut i = 0;
		let mut dropn = 0;
		let mut delst = dian.hashset();
		while i < len {
			let id = DiamondName::from(form[i*DS..i*DS+DS].try_into().unwrap());
			if delst.contains(&id) {
				dropn += 1;
				delst.remove(&id);
				len -= 1;
				// Only copy if i < len (there's a valid element after deletion)
				// When deleting the last element (i == len-1), len becomes len-1, and i == len, so no copy needed
				if i < len {
					form.copy_within(len*DS..len*DS+DS, i*DS);
				}
				if delst.is_empty() { break }
				// Note: Don't increment i here because the element at position i has been replaced
				// We need to check the new element at position i again
			} else {
				i += 1;
			}
		}
		// System-level invariant: all requested diamonds must be found
		if !delst.is_empty() {
			return errf!("DiamondOwnedForm drop: some diamonds {} not found in form, found {}, requested {}", 
				dian.readable(), dropn, dian.length())
		}
		assert!(dropn == dian.length(), "DiamondOwnedForm need drop {} but do {}, drop {}", 
			dian.length(), dropn, dian.readable());
		form.truncate(len * DS);
		self.names = BytesW4::from(form)?;
		Ok(dropn)
	}


}



