

combi_struct!{ PosiHash,
    posi: Uint1
    hash: Hash
}


combi_list!{ MerkelStuffs,
    Uint1, PosiHash
}


pub struct UnlockScript {
    stuff: Vec<u8>,
    witness: Vec<u8>,
}

/// Result of `scriptmh` address derivation for a P2SH lock script.
///
/// This is intended to help wallet / tooling authors compute the correct `SCRIPTMH` address
/// deterministically, without re-implementing the hashing rules and accidentally diverging
/// from consensus.
///
/// Hashing rules (same as `UnlockScriptProve::get_merkel()`):
/// - Leaf: `sha3("p2sh_leaf_" || libs || lockbox)`
/// - Branch i: `sha3("p2sh_branch_" || left || right)` where `(left,right)` is decided by `posi`.
/// - Address: `Address::create_scriptmh(ripemd160(root_sha3))`
#[derive(Debug, Clone)]
pub struct ScriptmhCalc {
    /// Final `SCRIPTMH` address (base58 leading symbol usually `3`).
    pub address: Address,
    /// `ripemd160(root_sha3)` that becomes the address payload (20 bytes).
    pub payload20: [u8; 20],
    /// SHA3-256 chain. `sha3_path[0]` is the leaf hash, `sha3_path.last()` is the root hash.
    pub sha3_path: Vec<Hash>,
}


/* pay to script hash */
action_define!{UnlockScriptProve, 90, 
    ActLv::Ast, // level
    false, [],
    {
        // calc hash: script + calibs
        argvkey: BytesW2 // unlock witness bytes (not executed)
        lockbox: BytesW2 // verify bytecodes
        adrlibs: ContractAddressW1 // lib address list for pure and callcode call
        merkels: MerkelStuffs
        _marks_: Fixed2
    },
    (self, "Prove P2SH unlock script".to_owned()),
    (self, ctx, _gas {
        if self._marks_.not_zero() {
            return errf!("marks bytes format error")
        }
        let adr = self.get_merkel()?;
        ctx.p2sh_set(adr, Box::new(self.get_stuff(ctx)?))?;
        // finish
        Ok(vec![])
    })
}


impl P2sh for UnlockScript {
    fn code_stuff(&self) -> &[u8] {
        &self.stuff
    }
    fn witness(&self) -> &[u8] {
        &self.witness
    }
}


impl UnlockScriptProve {

    /// Compute the `SCRIPTMH` address from:
    /// - `adrlibs`: the contract library allowlist used by the P2SH lock script
    /// - `lockbox`: the P2SH lock script bytecode (as it appears in this action field)
    /// - `merkels`: the Merkle proof path (siblings + left/right positions) used to commit
    ///   the lock script into a Merkle root.
    ///
    /// This helper intentionally returns intermediate hashes (`sha3_path`) so that tools can:
    /// - debug address derivation step-by-step
    /// - display/verify the Merkle proof path
    /// - avoid subtle encoding mistakes
    ///
    /// Notes for tooling authors:
    /// - `lockbox` is hashed as its field serialization bytes (i.e. `BytesW2::to_vec()`),
    ///   not as a custom encoding.
    /// - Each sibling `hash` is hashed as `hash.serialize()` (type `field::Hash`).
    pub fn calc_scriptmh_from_lockbox(adrlibs: &ContractAddressW1, lockbox: &BytesW2, merkels: &MerkelStuffs) -> Ret<ScriptmhCalc> {
        let mut h = Hash::from(sha3(vec![
            "p2sh_leaf_".as_bytes().to_vec(), // domain separator for safety
            adrlibs.serialize(),
            lockbox.to_vec(),
        ].concat()));
        let mut path = vec![h.clone()];
        for step in merkels.list().iter() {
            let posi = step.posi.uint();
            if posi > 1 {
                return errf!("p2sh merkel posi {} invalid, must be 0 or 1", posi)
            }
            let ch = h.clone();
            // left or right: posi==0 means sibling on the left, posi==1 means sibling on the right.
            let pair = maybe!(posi == 0, [step.hash, ch], [ch, step.hash]);
            let mut buf: Vec<_> = pair.iter().map(|a|a.serialize()).collect();
            buf.insert(0, "p2sh_branch_".as_bytes().to_vec()); // domain separator for safety
            h = Hash::from(sha3(buf.concat()));
            path.push(h.clone());
        }
        let payload20 = ripemd160(h);
        Ok(ScriptmhCalc{
            address: Address::create_scriptmh(payload20),
            payload20,
            sha3_path: path,
        })
    }

    fn verify_witness_bytes(cap: &SpaceCap, witness: &[u8]) -> Ret<()> {
        if witness.len() > cap.max_value_size {
            return errf!("p2sh witness bytes too long")
        }
        Ok(())
    }

    fn get_stuff(&self, ctx: &dyn Context) -> Ret<UnlockScript> {
        // check bytecodes
        let hei = ctx.env().block.height;
        let cap = SpaceCap::new(hei);
        // check libs all is contract 
        let libs = self.adrlibs.list();
        if libs.len() > cap.librarys_link {
            return errf!("p2sh libs overflow ({}>{})", libs.len(), cap.librarys_link)
        }
        if ! libs.iter().all(|a|a.is_contract()) {
            return errf!("contract libs error")
        }
        let ctb = CodeType::Bytecode;
        let lockbox = self.lockbox.as_vec();
        let witness = self.argvkey.as_vec().clone();
        convert_and_check(&cap, ctb, &lockbox, hei)?;
        Self::verify_witness_bytes(&cap, &witness)?;
        // ok
        let merkel = self.get_merkel()?.to_vec();
        let libs = self.adrlibs.serialize();
        let mut stuff = Vec::with_capacity(
            merkel.len() + libs.len() + lockbox.len()
        );
        stuff.extend_from_slice(&merkel);
        stuff.extend_from_slice(&libs);
        stuff.extend_from_slice(&lockbox);
        Ok(UnlockScript{ stuff, witness })
    }

    fn get_merkel(&self) -> Ret<Address> {
        Ok(Self::calc_scriptmh_from_lockbox(&self.adrlibs, &self.lockbox, &self.merkels)?.address)
    }
    
}
