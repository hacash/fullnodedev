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
	fncnf: Fixed1 // abstract call flags, currently must be zero
    code_stuff: CodeStuff
}

// Contract User Func
combi_struct!{ ContractUserFunc, 
	sign: Fixed4
	mark: Fixed3
	fncnf: Fixed1 // function flags, e.g. external
	pmdf: FuncArgvTypes // params type define
    code_stuff: CodeStuff
}

combi_struct!{ ContractCalcFunc,
	sign: Fixed4
	mark: Fixed1
	fncnf: Fixed1
	code_stuff: CodeStuff
}

// Contract address list
combi_list!(ContractAddrListW1, Uint1, ContractAddress);

impl ContractAddrListW1 {
	pub fn check_repeat(&self, src: &Self) -> bool {
		self.lists.iter().any(|a|src.lists.contains(a))
	}
}

// Func List
combi_list!(ContractAbstCallList, Uint1, ContractAbstCall);
combi_list!(ContractUserFuncList, Uint2, ContractUserFunc);
combi_list!(ContractCalcFuncList, Uint2, ContractCalcFunc);


// Replace At
combi_struct!{ ContractAddrReplaceAt,
	idx: Uint1
	addr: ContractAddress
}

combi_list!(ContractAddrReplaceAtList, Uint1, ContractAddrReplaceAt);

combi_struct!{ ContractEdition,
	revision: Uint2
	raw_size: Uint4
	hash: Hash
}

impl Copy for ContractEdition {}


// Contract Edit
combi_struct!{ ContractEdit,
	new_revision: Uint2
	inherit_add:  ContractAddrListW1
	inherit_replace_at: ContractAddrReplaceAtList
	library_add:  ContractAddrListW1
	library_replace_at: ContractAddrReplaceAtList
	abstcalls: ContractAbstCallList
	userfuncs: ContractUserFuncList
	calcfuncs: ContractCalcFuncList
}


impl ContractMeta {

	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract format invalid");
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
		let e = itr_err_fmt!(ContractError, "contract ContractAbstCall format invalid");
		if self.mark.not_zero() {
			return e
		}
		if self.fncnf.not_zero() {
			return e
		}
		Ok(())
	}
}

impl ContractUserFunc {
	
	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract ContractUserFunc format invalid");
		if self.mark.not_zero() {
			return e
		}
		let known = FnConf::External as u8;
		if self.fncnf[0] & !known != 0 {
			return e
		}
		Ok(())
	}
}

impl ContractCalcFunc {

	#[allow(dead_code)]
	fn check(&self, _hei: u64) -> VmrtErr {
		let e = itr_err_fmt!(ContractError, "contract ContractCalcFunc format invalid");
		if self.mark.not_zero() {
			return e
		}
		if self.fncnf.not_zero() {
			return e
		}
		Ok(())
	}
}

fn verify_code_stuff(cap: &SpaceCap, gst: &GasExtra, code_stuff: &CodeStuff, hei: u64) -> VmrtErr {
	let code_pkg = CodePkg::try_from(code_stuff)?;
	convert_and_check(cap, gst, code_pkg.code_type()?, &code_pkg.data, hei)?;
	Ok(())
}

macro_rules! func_list_merge_define {
	($ty:ty) => {
		// return edit or push
		fn addition(&mut self, func: $ty) -> Ret<bool> {
			let list = self.as_list();
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
			for a in src.as_list() {
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
	inherit:  ContractAddrListW1
	library:  ContractAddrListW1
	abstcalls: ContractAbstCallList
	userfuncs: ContractUserFuncList
	calcfuncs: ContractCalcFuncList
	morextend: Uint8
}


impl ContractSto {

	pub fn calc_edition(&self) -> ContractEdition {
		ContractEdition {
			revision: self.metas.revision,
			raw_size: Uint4::from(self.size() as u32),
			hash: Hash::from(sha3(self.serialize())),
		}
	}

	pub fn apply_edit(&mut self, edit: &ContractEdit, hei: u64, cap: &SpaceCap, gst: &GasExtra) -> VmrtRes<bool> {
		use ItrErrCode::*;

		let old_rev = self.metas.revision.uint();
		let Some(next_rev) = old_rev.checked_add(1) else {
			return itr_err_fmt!(ContractError, "contract revision overflow");
		};
		if edit.new_revision.uint() != next_rev {
			return itr_err_fmt!(
				ContractError,
				"contract revision mismatch: requested new_revision {} but next revision must be {}",
				edit.new_revision.uint(),
				next_rev
			);
		}

		let edit_empty = edit.inherit_add.length() == 0
			&& edit.inherit_replace_at.length() == 0
			&& edit.library_add.length() == 0
			&& edit.library_replace_at.length() == 0
			&& edit.abstcalls.length() == 0
			&& edit.userfuncs.length() == 0
			&& edit.calcfuncs.length() == 0;
		if edit_empty {
			return itr_err_fmt!(ContractError, "contract edit is empty");
		}
		if edit.calcfuncs.length() > 0 {
			return itr_err_fmt!(ContractError, "calcfunc not enabled yet");
		}

		let mut did_change = false;

		let inh_len = self.inherit.length();
		let lib_len = self.library.length();

		if edit.inherit_replace_at.length() > 0 {
			did_change = true;
			let mut idxs = HashSet::new();
			for r in edit.inherit_replace_at.as_list() {
				r.addr.check().map_ire(ContractAddrErr)?;
				let idx = r.idx.uint() as usize;
				if !idxs.insert(r.idx.uint()) {
					return itr_err_fmt!(InheritError, "inherit replace index already exists")
				}
				if idx >= inh_len {
					return itr_err_fmt!(InheritError, "inherit replace index overflow")
				}
				self.inherit.replace(idx, r.addr.clone()).map_ire(InheritError)?;
			}
		}
		if edit.library_replace_at.length() > 0 {
			did_change = true;
			let mut idxs = HashSet::new();
			for r in edit.library_replace_at.as_list() {
				r.addr.check().map_ire(ContractAddrErr)?;
				let idx = r.idx.uint() as usize;
				if !idxs.insert(r.idx.uint()) {
					return itr_err_fmt!(LibraryError, "library replace index already exists")
				}
				if idx >= lib_len {
					return itr_err_fmt!(LibraryError, "library replace index overflow")
				}
				self.library.replace(idx, r.addr.clone()).map_ire(LibraryError)?;
			}
		}

		if self.inherit.length() + edit.inherit_add.length() > cap.inherit {
			return itr_err_fmt!(InheritError, "inherit number overflow")
		}
		if self.library.length() + edit.library_add.length() > cap.library {
			return itr_err_fmt!(LibraryError, "library link number overflow")
		}

		if edit.inherit_add.length() > 0 {
			for a in edit.inherit_add.as_list() {
				a.check().map_ire(ContractAddrErr)?;
			}
			self.inherit.append(edit.inherit_add.lists.clone()).map_ire(InheritError)?;
		}
		if edit.library_add.length() > 0 {
			for a in edit.library_add.as_list() {
				a.check().map_ire(ContractAddrErr)?;
			}
			self.library.append(edit.library_add.lists.clone()).map_ire(LibraryError)?;
		}

		if edit.abstcalls.length() > 0 {
			did_change = true;
			{
				let mut seen = HashSet::new();
				for a in edit.abstcalls.as_list() {
					if a.sign[0] == AbstCall::Construct as u8 {
						return itr_err_fmt!(
							ContractUpgradeErr,
							"contract update cannot carry Construct abstcall"
						)
					}
					if !seen.insert(a.sign[0]) {
						return itr_err_fmt!(ContractUpgradeErr, "abstcall sign already exists in edit")
					}
				}
			}
			for a in edit.abstcalls.as_list() {
				a.check(hei)?;
				AbstCall::check(a.sign[0])?;
				verify_code_stuff(cap, gst, &a.code_stuff, hei)?;
			}
			self.abstcalls.check_merge(&edit.abstcalls)?;
		}

		if edit.userfuncs.length() > 0 {
			{
				let mut seen = HashSet::new();
				for a in edit.userfuncs.as_list() {
					let key = a.sign.to_array();
					if !seen.insert(key) {
						return itr_err_fmt!(ContractUpgradeErr, "userfunc sign already exists in edit")
					}
				}
			}
			for a in edit.userfuncs.as_list() {
				a.check(hei)?;
				verify_code_stuff(cap, gst, &a.code_stuff, hei)?;
			}
			if self.userfuncs.check_merge(&edit.userfuncs)? {
				did_change = true;
			}
		}

		self.metas.revision = Uint2::from(next_rev);

		// Final tail check: enforces size cap, dedup of inherit/library/abstcall/userfunc,
		// reserved-must-be-zero on `morextend`, and `calcfuncs.length() == 0`.
		self.check(hei, cap, gst)?;
		Ok(did_change)
	}


	pub fn have_abst_call(&self, ac: AbstCall) -> bool {
		for a in self.abstcalls.as_list() {
			if ac as u8 == a.sign[0] {
				return true
			}
		}
		false
	}

	pub fn check(&self, hei: u64, cap: &SpaceCap, gst: &GasExtra) -> VmrtErr {
		self.metas.check(hei)?;
		use ItrErrCode::*;
		// `morextend` and `calcfuncs` are reserved slots: kept in the on-disk layout for
		// forward compatibility but currently must be empty/zero.
		if self.morextend.uint() != 0 {
			return itr_err_fmt!(ContractError, "morextend reserved, must be zero")
		}
		if self.calcfuncs.length() != 0 {
			return itr_err_fmt!(ContractError, "calcfunc not enabled yet")
		}
		// check size
		if self.size() > cap.contract_size {
			return itr_err_fmt!(ContractError, "contract size overflow, max {}", cap.contract_size)
		}
		// inherit and library
		if self.inherit.length() > cap.inherit {
			return itr_err_fmt!(InheritError, "inherit number overflow")
		}
		if self.library.length() > cap.library {
			return itr_err_fmt!(LibraryError, "library link number overflow")
		}
		// inherit/library address version & no-duplicate check
		{
			let mut inhset: HashSet<ContractAddress> = HashSet::new();
			for a in self.inherit.as_list() {
				a.check().map_ire(ContractAddrErr)?;
				if !inhset.insert(a.clone()) {
					return itr_err_fmt!(InheritError, "inherit already exists")
				}
			}
			let mut libset: HashSet<ContractAddress> = HashSet::new();
			for a in self.library.as_list() {
				a.check().map_ire(ContractAddrErr)?;
				if !libset.insert(a.clone()) {
					return itr_err_fmt!(LibraryError, "library already exists")
				}
			}
		}
		// abst call
		{
			let mut seen = HashSet::new();
			for a in self.abstcalls.as_list() {
				if !seen.insert(a.sign[0]) {
					return itr_err_fmt!(ContractError, "abstcall sign already exists")
				}
			}
		}
		for a in self.abstcalls.as_list() {
			a.check(hei)?;
			AbstCall::check(a.sign[0])?;
			verify_code_stuff(cap, gst, &a.code_stuff, hei)?; // check compile
		}
		// usrfun call
		{
			let mut seen = HashSet::new();
			for a in self.userfuncs.as_list() {
				let key = a.sign.to_array();
				if !seen.insert(key) {
					return itr_err_fmt!(ContractError, "userfunc sign already exists")
				}
			}
		}
		for a in self.userfuncs.as_list() {
			a.check(hei)?;
			verify_code_stuff(cap, gst, &a.code_stuff, hei)?; // check compile
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
	pub edition: ContractEdition,
}


impl ContractSto {

	pub fn into_obj(mut self) -> VmrtRes<ContractObj> {
		let edition = self.calc_edition();
		let mut abstfns = HashMap::with_capacity(self.abstcalls.length());
		// Move function bytecode out of `ContractSto` once. Runtime execution uses `FnObj`, so keeping another full copy inside `sto` only adds memory and copy cost.
		for a in self.abstcalls.as_mut() {
			let code_pkg = CodePkg::try_from(std::mem::take(&mut a.code_stuff))?;
			let code = FnObj::create(a.fncnf[0], code_pkg, None)?;
			let cty = AbstCall::try_from_u8(a.sign[0])?;
			abstfns.insert(cty, code.into());
		}
		let mut userfns = HashMap::with_capacity(self.userfuncs.length());
		for a in self.userfuncs.as_mut() {
			let code_pkg = CodePkg::try_from(std::mem::take(&mut a.code_stuff))?;
			let code = FnObj::create(a.fncnf[0], code_pkg, Some(a.pmdf.clone()))?;
			let cty = a.sign.to_array();
			userfns.insert(cty, code.into());
		}
		// `calcfuncs` is a reserved on-disk slot kept for forward compatibility; the
		// list is enforced empty by `ContractSto::check`, so there is nothing to load here.
		Ok(ContractObj {
			sto: self,
			abstfns,
			userfns,
			edition,
		})
	}
}

#[cfg(test)]
mod contract_obj_tests {
	use super::*;

	#[test]
	fn into_obj_keeps_original_edition_before_taking_code() {
		let mut sto = ContractSto::new();
		let mut f = ContractUserFunc::new();
		f.sign = Fixed4::from([1u8, 2, 3, 4]);
		let pkg = CodePkg {
			conf: CodeConf::from_type(CodeType::Bytecode).raw(),
			data: vec![Bytecode::END as u8],
		};
		f.code_stuff = CodeStuff::try_from(pkg).unwrap();
		sto.userfuncs.push(f).unwrap();
		let raw = sto.size() as u32;
		let hx = Hash::from(sha3(sto.serialize()));
		let obj = sto.into_obj().unwrap();
		assert_eq!(obj.edition.raw_size.uint(), raw);
		assert_eq!(obj.edition.hash, hx);
	}
}
