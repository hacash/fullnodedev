

// pub const CONTRACT_STORE_FEE_MUL: u64 = 50;
pub const CONTRACT_STORE_PERM_PERIODS: u64 = 10_000;


macro_rules! vmsto {
    ($ctx: expr) => {
        VMState::wrap($ctx.state())
    };
}



action_define!{ContractDeploy, 99, 
    ActLv::TopOnlyWithGuard,
    false, [],
    {   
        protocol_cost: Amount
        nonce: Uint4 
        construct_argv: BytesW1 // max 1024
        _marks_:   Fixed4 // zero
        contract: ContractSto
    },
    (self, {
        format!("Deploy smart contract with nonce {}", *self.nonce)
    }),
    (self, ctx, _gas {
        if self._marks_.not_zero() { // compatibility for future
            return errf!("marks byte error")
        }
        let hei = ctx.env().block.height;
        let maddr = ctx.env().tx.main;
        // check contract
        let caddr = ContractAddress::calculate(&maddr, &self.nonce);
        if vmsto!(ctx).contract_exist(&caddr) {
            return errf!("contract {} already exist", (*caddr).to_readable())
        }
        // cannot inherit self or link self as library
        if self.contract.inherits.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot inherit itself {}", (*caddr).to_readable())
        }
        if self.contract.librarys.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot link itself as library {}", (*caddr).to_readable())
        }
        // check
        self.contract.check(hei)?;
        if self.contract.metas.revision.uint() != 0 {
            return errf!("contract revision must be 0 on deploy")
        }
        let accf  = AbstCall::Construct;
        let hvaccf = self.contract.have_abst_call(accf);
        let charge_bytes = self.contract.size();
        // spend protocol fee
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, charge_bytes)?;
        // save the contract
        vmsto!(ctx).contract_set(&caddr, &self.contract);
        // call the construct function
        let cargv = self.construct_argv.to_vec();
        if cargv.len() > SpaceCap::new(hei).max_value_size {
            return errf!("construct argv size overflow")
        }
        if hvaccf { // have Construct func
            let cty = ExecMode::Abst as u8;
            setup_vm_run(ctx, cty, accf as u8, caddr.as_bytes(), Value::Bytes(cargv))?;
        }
        // ok finish
        Ok(vec![])
    })
}






action_define!{ContractUpdate, 98, 
    ActLv::TopOnlyWithGuard, // level
    false, [], // burn 90% fee
    {   
        protocol_cost: Amount
        address: Address // contract address
        _marks_: Fixed2 // zero
        edit: ContractEdit
    },
    (self, format!("Update smart contract {}", self.address)),
    (self, ctx, _gas {
        use AbstCall::*;
        if self._marks_.not_zero() {
            return errf!("marks byte error")
        }
        let hei = ctx.env().block.height;
        // load old
        let caddr = ContractAddress::from_addr(self.address)?;
        let Some(contract) = vmsto!(ctx).contract(&caddr) else {
            return errf!("contract {} not exist", (*caddr).to_readable())
        };
        // apply edit (in memory)
		let mut new_contract = contract.clone();
        let (_did_append, did_change) = new_contract.apply_edit(&self.edit, hei)?;
        // cannot inherit self or link self as library
        if new_contract.inherits.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot inherit itself {}", (*caddr).to_readable())
        }
        if new_contract.librarys.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot link itself as library {}", (*caddr).to_readable())
        }
        // spend protocol fee only when storage grows
        let old_size = contract.size();
        let new_size = new_contract.size();
        let delta_bytes = new_size.saturating_sub(old_size);
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, delta_bytes)?;
        let cty = ExecMode::Abst as u8;
        let sys = maybe!(did_change, Change, Append) as u8; // Change or Append
        setup_vm_run(ctx, cty, sys, caddr.as_bytes(), Value::Nil)?;
        // save the new
        vmsto!(ctx).contract_set(&caddr, &new_contract);
        Ok(vec![]) 
    })
}




/**************************************/



fn check_sub_contract_protocol_fee(ctx: &mut dyn Context, pfee: &Amount, charge_bytes: usize) -> Rerr {
    if pfee.is_negative() {
		return errf!("protocol fee cannot be negative")
    }
    if charge_bytes == 0 {
        return Ok(())
    }
    let min_fee = calc_contract_protocol_fee_min(ctx, charge_bytes)?;
    let maddr = ctx.env().tx.main;
    // check fee
    if pfee < &min_fee { 
        return errf!(
            "protocol fee must need at least {} (bytes={}, periods={}) but just got {}",
            &min_fee,
            charge_bytes,
            contract_store_perm_periods(ctx.env().block.height),
            pfee
        )
    }
    operate::hac_sub(ctx, &maddr, pfee)?;
    Ok(())
}

#[inline(always)]
fn contract_store_perm_periods(_hei: u64) -> u64 {
    // Keep this as a function to make future fork-by-height tuning low-coupling.
    CONTRACT_STORE_PERM_PERIODS
}

fn calc_contract_protocol_fee_min(ctx: &dyn Context, charge_bytes: usize) -> Ret<Amount> {
    if charge_bytes == 0 {
        return Ok(Amount::zero())
    }
    let periods = contract_store_perm_periods(ctx.env().block.height) as u128;
    let fee_purity = ctx.tx().fee_purity() as u128; // unit-238 per tx byte
    if periods == 0 || fee_purity == 0 {
        return errf!(
            "contract protocol fee calculate failed: periods={} fee_purity={}",
            periods,
            fee_purity
        )
    }
    let bytes = charge_bytes as u128;
    let Some(need) = fee_purity.checked_mul(bytes) else {
        return errf!(
            "contract protocol fee calculate failed: fee_purity * bytes overflow ({} * {})",
            fee_purity,
            bytes
        )
    };
    let Some(need) = need.checked_mul(periods) else {
        return errf!(
            "contract protocol fee calculate failed: need * periods overflow ({} * {})",
            need,
            periods
        )
    };
    Ok(Amount::coin_u128(need, UNIT_238))
}




/**************************************

fn check_sub_contract_protocol_fee(ctx: &mut dyn Context, ctlsz: usize, ptcfee: &Amount) -> Rerr {
    // let _hei = ctx.env().block.height;
    let e = errf!("contract protocol fee calculate failed");
    let mul = CONTRACT_STORE_FEE_MUL as u128; // 30
    let feep = ctx.tx().fee_purity() as u128; // per-byte, no GSCU division
    let Some(rlfe) = feep.checked_mul(ctlsz as u128) else {
        return e
    };
    let Some(rlfe) = rlfe.checked_mul(mul) else {
        return e
    };
    let tx50fee = &Amount::coin_u128(rlfe, UNIT_238).compress(2, AmtCpr::Grow)?;
    if tx50fee <= ctx.tx().fee() {
        return e
    }
    println!("{}, {}, {}, {}", ctx.tx().size(), ctlsz, ctx.tx().fee(), tx50fee);
    let maddr = ctx.env().tx.main;
    // check fee
    if ptcfee < tx50fee { 
        return errf!("protocol fee must need at least {} but just got {}", tx50fee, ptcfee)
    }
    operate::hac_sub(ctx, &maddr, ptcfee)?;
    Ok(())
}


*/
