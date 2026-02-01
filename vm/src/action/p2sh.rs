

combi_struct!{ PosiHash,
    posi: Uint1
    hash: Hash
}


combi_list!{ MerkelStuffs,
    Uint1, PosiHash
}


pub struct UnlockScript {
    stuff: Vec<u8>
}


/*
    pay to script hash
*/
action_define!{UnlockScriptProve, 97, 
    ActLv::Ast, // level
    false, [],
    {
        // calc hash: script + calibs
        argvkey: BytesW2 // unlock bytecodes
        lockbox: BytesW2 // verify bytecodes
        adrlibs: ContractAddressW1 // lib address list for pure and callcode call
        merkels: MerkelStuffs
        _marks_: Fixed2
    },
    (self, "Prove P2SH unlock script".to_owned()),
    (self, ctx, _gas {
        #[cfg(not(feature = "p2sh"))]
        if true {
            return errf!("p2sh not yet")
        }
        if self._marks_.not_zero() {
            return errf!("marks bytes format error")
        }
        let adr = self.get_merkel();
        ctx.p2sh_set(adr, Box::new(self.get_stuff(ctx)?))?;
        // finish
        Ok(vec![])
    })
}


impl P2sh for UnlockScript {
    fn code_stuff(&self) -> &[u8] {
        &self.stuff
    }
}


impl UnlockScriptProve {

    fn get_stuff(&self, ctx: &dyn Context) -> Ret<UnlockScript> {
        // check libs all is contract 
        if ! self.adrlibs.list().iter().all(|a|a.is_contract()) {
            return errf!("contract libs error")
        }
        // check bytecodes
        let cap = SpaceCap::new(ctx.env().block.height);
        let ctb = CodeType::Bytecode;
        let lockbox = self.lockbox.as_vec();
        let unlocks = self.argvkey.as_vec();
        convert_and_check(&cap, ctb, &lockbox)?;
        let insts = convert_and_check(&cap, ctb, &unlocks)?;
        if unlocks.len() + lockbox.len() > cap.one_function_size {
            return errf!("p2sh code too long")
        }
        // check unlock no return or non-stack write
        use Bytecode::*;
        if unlocks.iter().enumerate().any(|(i, a)|{
            if 0 == insts[i] {
                return false // data seg
            }
            let inst: Bytecode = std_mem_transmute!(*a);
            match inst {
                RET | END |
                SDEL | SSAVE | SRENT |
                GPUT | MPUT |
                LOG1 | LOG2 | LOG3 | LOG4 |
                HWRITE | HWRITEX | HWRITEXL | HGROW |
                ALLOC | PUT | PUTX | XOP | XLG | UPLIST |
                CALL | CALLTHIS | CALLSELF | CALLSUPER | CALLVIEW | CALLCODE | CALLPURE |
                EXTACTION => true,
                _ => false,
            }
        }) {
            return errf!("p2sh unlock script cannot return early or write outside stack")
        }
        // ok 
        let merkel = self.get_merkel().to_vec();
        let libs = self.adrlibs.serialize();
        let mut stuff = Vec::with_capacity(
            merkel.len() + libs.len() + unlocks.len() + lockbox.len()
        );
        stuff.extend_from_slice(&merkel);
        stuff.extend_from_slice(&libs);
        stuff.extend_from_slice(unlocks);
        stuff.extend_from_slice(lockbox);
        Ok(UnlockScript{ stuff })
    }

    fn get_merkel(&self) -> Address {
        let mut hash = Hash::from(sha3(vec![
            "p2sh_leaf_".as_bytes().to_vec(), // tag for safe
            self.adrlibs.serialize(),
            self.lockbox.to_vec(),
        ].concat()));
        // 
        for h in self.merkels.list().iter() {
            let ch = hash.clone();
            // left or right
            let stf = maybe!( h.posi.uint()==0, [h.hash, ch], [ch, h.hash] );
            let mut stf: Vec<_> = stf.iter().map(|a|a.serialize()).collect();
            stf.insert(0, "p2sh_branch_".as_bytes().to_vec()); // tag for safe
            hash = Hash::from(sha3(stf.concat()));
        }
        let hs20 = ripemd160(hash);
        Address::create_scriptmh(hs20)
    }
    
}
