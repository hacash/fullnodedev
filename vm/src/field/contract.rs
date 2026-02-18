use std::collections::*;



// Contract Head
combi_struct!{ ContractMeta, 
    vrsn: Fixed1 // 4bit16 = version
	revision: Uint2
	mark: Fixed3
	mext: Fixed4
}

// Contract Abst Call
combi_struct!{ ContractAbstCall, 
	sign: Fixed1
	mark: Fixed2
	cdty: Fixed1 // 3bit8 = codetype
    code: BytesW2
}

// Contract User Func
combi_struct!{ ContractUserFunc, 
	sign: Fixed4
	mark: Fixed3
	cdty: Fixed1 // 1bit = is_public, 3bit8 = codetype
	pmdf: FuncArgvTypes // params type define
    code: BytesW2
}

// Contract address list
combi_list!(ContractAddrsssW1, Uint1, ContractAddress);

impl ContractAddrsssW1 {
	pub fn check_repeat(&self, src: &Self) -> bool {
		self.lists.iter().any(|a|src.lists.contains(a))
	}
}

// Func List
combi_list!(ContractAbstCallList, Uint1, ContractAbstCall);
combi_list!(ContractUserFuncList, Uint2, ContractUserFunc);


// Replace At
combi_struct!{ LibReplaceAt,
	idx: Uint1
	addr: ContractAddress
}

combi_list!(LibReplaceAtList, Uint1, LibReplaceAt);


// Contract Edit
combi_struct!{ ContractEdit,
	expect_revision: Uint2
	inherits_add:  ContractAddrsssW1
	inherits_replace_at: LibReplaceAtList
	librarys_add:  ContractAddrsssW1
	librarys_replace_at: LibReplaceAtList
	abstcalls: ContractAbstCallList
	userfuncs: ContractUserFuncList
}


impl ContractMeta {

	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract format error");
		if self.vrsn.not_zero() |
			self.mark.not_zero() |
			self.mext.not_zero()
		{
			return e
		}

		Ok(())
	}
}

impl ContractAbstCall {
	
	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract ContractAbstCall format error");
		if self.mark.not_zero() {
			return e
		}
		let c = self.cdty[0] >> 3;
		if c > 0 {
			return e
		}
		Ok(())
	}
}

impl ContractUserFunc {
	
	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract ContractUserFunc format error");
		if self.mark.not_zero() {
			return e
		}
		Ok(())
	}
}


macro_rules! func_list_merge_define {
	($ty:ty) => {
		// return edit or push
		fn addition(&mut self, func: $ty) -> Ret<bool> {
			let list = self.list();
			for i in 0..self.length() {
				if list[i].sign == func.sign {
					self.replace(i, func)?;
					return Ok(true)
				}
			}
			// push
			self.push(func)?;
			Ok(false)
		}
		// return edit or push
		fn check_merge(&mut self, src: &Self) -> VmrtRes<bool> {
			let mut edit = false;
			for a in src.list() {
				if self.addition(a.clone()).map_ire(ContractUpgradeErr)? {
					edit = true;
				}
			}
			Ok(edit)
		}
	};
}


impl ContractAbstCallList {
	func_list_merge_define!{ ContractAbstCall }
}

impl ContractUserFuncList {
	func_list_merge_define!{ ContractUserFunc }
}

// Contract
combi_struct!{ ContractSto, 
	metas: ContractMeta
	inherits:  ContractAddrsssW1
    librarys:  ContractAddrsssW1
	abstcalls: ContractAbstCallList
	userfuncs: ContractUserFuncList
    morextend: Uint8
}


impl ContractSto {

	pub fn apply_edit(&mut self, edit: &ContractEdit, hei: u64) -> VmrtRes<(bool, bool)> {
		use ItrErrCode::*;
		let cap = SpaceCap::new(hei);

		let old_rev = self.metas.revision.uint();
		if old_rev == Uint2::MAX {
			return itr_err_fmt!(ContractError, "contract revision reach max");
		}
		let Some(next_rev) = old_rev.checked_add(1) else {
			return itr_err_fmt!(ContractError, "contract revision overflow");
		};
		if edit.expect_revision.uint() != next_rev {
			return itr_err_fmt!(ContractError, "contract revision expect {} but need {}", edit.expect_revision.uint(), next_rev);
		}

		let edit_empty = edit.inherits_add.length() == 0
			&& edit.inherits_replace_at.length() == 0
			&& edit.librarys_add.length() == 0
			&& edit.librarys_replace_at.length() == 0
			&& edit.abstcalls.length() == 0
			&& edit.userfuncs.length() == 0;
		if edit_empty {
			return itr_err_fmt!(ContractError, "contract edit empty");
		}

		let mut did_append = false;
		let mut did_change = false;

		let inh_len = self.inherits.length();
		let lib_len = self.librarys.length();

		if edit.inherits_replace_at.length() > 0 {
			did_change = true;
			let mut idxs = HashSet::new();
			for r in edit.inherits_replace_at.list() {
				r.addr.check().map_ire(ContractAddrErr)?;
				let idx = r.idx.uint() as usize;
				if !idxs.insert(r.idx.uint()) {
					return itr_err_fmt!(InheritsError, "inherits replace index duplicated")
				}
				if idx >= inh_len {
					return itr_err_fmt!(InheritsError, "inherits replace index overflow")
				}
				self.inherits.replace(idx, r.addr.clone()).map_ire(InheritsError)?;
			}
		}
		if edit.librarys_replace_at.length() > 0 {
			did_change = true;
			let mut idxs = HashSet::new();
			for r in edit.librarys_replace_at.list() {
				r.addr.check().map_ire(ContractAddrErr)?;
				let idx = r.idx.uint() as usize;
				if !idxs.insert(r.idx.uint()) {
					return itr_err_fmt!(LibrarysError, "librarys replace index duplicated")
				}
				if idx >= lib_len {
					return itr_err_fmt!(LibrarysError, "librarys replace index overflow")
				}
				self.librarys.replace(idx, r.addr.clone()).map_ire(LibrarysError)?;
			}
		}

		if self.inherits.length() + edit.inherits_add.length() > cap.inherits_parent {
			return itr_err_fmt!(InheritsError, "inherits number overflow")
		}
		if self.librarys.length() + edit.librarys_add.length() > cap.librarys_link {
			return itr_err_fmt!(LibrarysError, "librarys link number overflow")
		}

		if edit.inherits_add.length() > 0 {
			for a in edit.inherits_add.list() {
				a.check().map_ire(ContractAddrErr)?;
			}
			self.inherits.append(edit.inherits_add.lists.clone()).unwrap();
			did_append = true;
		}
		if edit.librarys_add.length() > 0 {
			for a in edit.librarys_add.list() {
				a.check().map_ire(ContractAddrErr)?;
			}
			self.librarys.append(edit.librarys_add.lists.clone()).unwrap();
			did_append = true;
		}

		// check repeat after edit
		let mut inhset: HashSet<ContractAddress> = HashSet::new();
		for a in self.inherits.list() {
			if !inhset.insert(a.clone()) {
				return itr_err_fmt!(InheritsError, "inherits cannot repeat")
			}
		}
		let mut libset: HashSet<ContractAddress> = HashSet::new();
		for a in self.librarys.list() {
			if !libset.insert(a.clone()) {
				return itr_err_fmt!(LibrarysError, "librarys cannot repeat")
			}
		}

		if edit.abstcalls.length() > 0 {
			did_change = true;
			{
				let mut seen = HashSet::new();
				for a in edit.abstcalls.list() {
					if !seen.insert(a.sign[0]) {
						return itr_err_fmt!(ContractUpgradeErr, "abstcall sign repeat in edit")
					}
				}
			}
			for a in edit.abstcalls.list() {
				a.check(hei)?;
				AbstCall::check(a.sign[0])?;
				let ctype = CodeType::parse(a.cdty[0])?;
				convert_and_check(&cap, ctype, &a.code, hei)?;
			}
			self.abstcalls.check_merge(&edit.abstcalls)?;
		}

		if edit.userfuncs.length() > 0 {
			{
				let mut seen = HashSet::new();
				for a in edit.userfuncs.list() {
					let key = a.sign.to_array();
					if !seen.insert(key) {
						return itr_err_fmt!(ContractUpgradeErr, "userfunc sign repeat in edit")
					}
				}
			}
			for a in edit.userfuncs.list() {
				a.check(hei)?;
				let ctype = CodeType::parse(a.cdty[0])?;
				convert_and_check(&cap, ctype, &a.code, hei)?;
			}
			let replaced = self.userfuncs.check_merge(&edit.userfuncs)?;
			if replaced {
				did_change = true;
			} else {
				did_append = true;
			}
		}

		if self.size() > cap.max_contract_size {
			return itr_err_fmt!(ContractError, "contract size overflow, max {}", cap.max_contract_size)
		}

		self.metas.revision = Uint2::from(next_rev);
		Ok((did_append, did_change))
	}


	pub fn have_abst_call(&self, ac: AbstCall) -> bool {
		for a in self.abstcalls.list() {
			if ac as u8 == a.sign[0] {
				return true
			}
		}
		false
	}

	pub fn drop_abst_call(&mut self, ac: AbstCall) -> bool {
		let mut k: Option<usize> = None;
		let funcs = self.abstcalls.list();
		for i in 0..funcs.len() {
			let a = &funcs[i];
			if ac as u8 == a.sign[0] {
				k = Some(i);
				break;
			}
		}
		if let Some(i) = k {
			self.abstcalls.drop(i).unwrap();
			return true
		}
		false

	}

	/* return Upgrade or Append for check */
	pub fn merge(&mut self, src: &ContractSto, hei: u64) -> VmrtRes<bool> {
		use ItrErrCode::*;
		src.check(hei)?;
		let cap = SpaceCap::new(hei);
		if self.inherits.length() + src.inherits.length() > cap.inherits_parent {
			return itr_err_fmt!(InheritsError, "inherits number overflow")
		}
		if self.librarys.length() + src.librarys.length() > cap.librarys_link {
			return itr_err_fmt!(LibrarysError, "librarys link number overflow")
		}
		// inhs and libs check repeat
		if self.inherits.check_repeat(&src.inherits) {
			return itr_err_fmt!(InheritsError, "inherits cannot repeat")
		}
		if self.librarys.check_repeat(&src.librarys) {
			return itr_err_fmt!(LibrarysError, "librarys cannot repeat")
		}
		// append inherits and librarys
		self.inherits.append(src.inherits.lists.clone()).unwrap();
		self.librarys.append(src.librarys.lists.clone()).unwrap();
		// merge abstcall & usrfun 
		let edit1 = self.abstcalls.check_merge(&src.abstcalls)?;
		let edit2 = self.userfuncs.check_merge(&src.userfuncs)?;
		// check size
		if self.size() > cap.max_contract_size {
			return itr_err_fmt!(ContractError, "contract size overflow, max {}", cap.max_contract_size)
		}
		// ok
		Ok(edit1 || edit2)
	}

	pub fn check(&self, hei: u64) -> VmrtErr {
		self.metas.check(hei)?;
		use ItrErrCode::*;
		let e = itr_err_fmt!(ContractError, "contract format error");
		let cap = SpaceCap::new(hei);
		// check
		if self.morextend.uint() > 0 {
			return e
		}
		// check size
		if self.size() > cap.max_contract_size {
			return itr_err_fmt!(ContractError, "contract size overflow, max {}", cap.max_contract_size)
		}
		if 0 != *self.morextend {
			return itr_err_fmt!(ContractError, "extend data format error")
		}
		// inherits_parent and librarys
		if self.inherits.length() > cap.inherits_parent {
			return itr_err_fmt!(InheritsError, "inherits number overflow")
		}
		if self.librarys.length() > cap.librarys_link {
			return itr_err_fmt!(LibrarysError, "librarys link number overflow")
		}
		// inherits/librarys address version & no-duplicate check
		{
			let mut inhset: HashSet<ContractAddress> = HashSet::new();
			for a in self.inherits.list() {
				a.check().map_ire(ContractAddrErr)?;
				if !inhset.insert(a.clone()) {
					return itr_err_fmt!(InheritsError, "inherits cannot repeat")
				}
			}
			let mut libset: HashSet<ContractAddress> = HashSet::new();
			for a in self.librarys.list() {
				a.check().map_ire(ContractAddrErr)?;
				if !libset.insert(a.clone()) {
					return itr_err_fmt!(LibrarysError, "librarys cannot repeat")
				}
			}
		}
		// abst call
		{
			let mut seen = HashSet::new();
			for a in self.abstcalls.list() {
				if !seen.insert(a.sign[0]) {
					return itr_err_fmt!(ContractError, "abstcall sign repeat")
				}
			}
		}
		for a in self.abstcalls.list() {
			a.check(hei)?;
			AbstCall::check(a.sign[0])?;
			let ctype = CodeType::parse(a.cdty[0])?;
			convert_and_check(&cap, ctype, &a.code, hei)?; // // check compile
		}
		// usrfun call
		{
			let mut seen = HashSet::new();
			for a in self.userfuncs.list() {
				let key = a.sign.to_array();
				if !seen.insert(key) {
					return itr_err_fmt!(ContractError, "userfunc sign repeat")
				}
			}
		}
		for a in self.userfuncs.list() {
			a.check(hei)?;
			let ctype = CodeType::parse(a.cdty[0])?;
			convert_and_check(&cap, ctype, &a.code, hei)?; // check compile
		}
		// ok
		Ok(())
	}
}


//////////////////////////////////////




#[derive(Default)]
pub struct ContractObj {
	pub sto: ContractSto,
	pub abstfns: HashMap<AbstCall, Arc<FnObj>>,
	pub userfns: HashMap<FnSign, Arc<FnObj>>,
}


impl ContractSto {

	pub fn into_obj(mut self) -> VmrtRes<ContractObj> {
		let mut abstfns = HashMap::with_capacity(self.abstcalls.length());
		// Move function bytecode out of `ContractSto` once. Runtime execution uses `FnObj`, so keeping another full copy inside `sto` only adds memory and copy cost.
		for a in self.abstcalls.as_mut() {
			let code_bytes = std::mem::take(&mut a.code).into_vec();
			let code = FnObj::create(a.cdty[0], code_bytes, None)?;
			let cty = std_mem_transmute!(a.sign[0]);
			abstfns.insert(cty, code.into());
		}
		let mut userfns = HashMap::with_capacity(self.userfuncs.length());
		for a in self.userfuncs.as_mut() {
			let code_bytes = std::mem::take(&mut a.code).into_vec();
			let code = FnObj::create(a.cdty[0], code_bytes, Some(a.pmdf.clone()))?;
			let cty = a.sign.to_array();
			userfns.insert(cty, code.into());
		}
		Ok(ContractObj { sto: self, abstfns, userfns })
	}
}

