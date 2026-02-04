

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

    fn verify_unlocks_bytecode(cap: &SpaceCap, unlocks: &[u8]) -> Ret<()> {
        use Bytecode::*;
        if unlocks.is_empty() {
            return errf!("p2sh unlock script cannot be empty")
        }
        // Use convert_and_check() but avoid the tail-END constraint by temporarily appending END.
        // Also ensure the appended END is a real instruction (not consumed as PBUF/PBUFL data),
        // otherwise a malformed unlock script could "borrow" this byte as missing data.
        let mut tmp = Vec::with_capacity(unlocks.len() + 1);
        tmp.extend_from_slice(unlocks);
        tmp.push(END as u8);
        let insts = convert_and_check(cap, CodeType::Bytecode, &tmp)?;
        if insts[unlocks.len()] == 0 {
            return errf!("p2sh unlock script tail format error")
        }
        // Disallow opcodes for safety:
        // - no jumps / branches
        // - no early exit (RET/END/ERR/ABT/AST)
        // - no state writes (global/memory/storage/log/heap grow/write)
        // - no contract calls (any CALL*)
        // Allowed: stack ops + pure computation + read-only state/storage (GGET/MGET/SLOAD/SREST) etc.
        for (i, b) in unlocks.iter().enumerate() {
            if insts[i] == 0 {
                continue // data segment
            }
            let inst: Bytecode = std_mem_transmute!(*b);
            match inst {
                // forbid control flow
                JMPL | JMPS | JMPSL | BRL | BRS | BRSL | BRSLN => {
                    return errf!("p2sh unlock script cannot use jump/branch opcode {}", *b)
                }
                // forbid early exit / abort
                RET | END | ERR | ABT | AST => {
                    return errf!("p2sh unlock script cannot return/abort early (opcode {})", *b)
                }
                // forbid any contract call
                CALL | CALLTHIS | CALLSELF | CALLSUPER | CALLVIEW | CALLPURE | CALLCODE => {
                    return errf!("p2sh unlock script cannot call contracts (opcode {})", *b)
                }
                // forbid state writes (but allow reads)
                SDEL | SSAVE | SRENT | GPUT | MPUT | LOG1 | LOG2 | LOG3 | LOG4 => {
                    return errf!("p2sh unlock script cannot write state/log (opcode {})", *b)
                }
                // forbid heap write/grow (outside operand stack)
                HWRITE | HWRITEX | HWRITEXL | HGROW => {
                    return errf!("p2sh unlock script cannot write heap (opcode {})", *b)
                }
                // forbid local write / outside-stack mutations
                ALLOC | PUT | PUTX | XOP | XLG | UPLIST => {
                    return errf!("p2sh unlock script cannot write outside operand stack (opcode {})", *b)
                }
                // forbid extend action
                EXTACTION => {
                    return errf!("p2sh unlock script cannot use EXTACTION (opcode {})", *b)
                }
                // debug / panic
                PRT | NT => {
                    return errf!("p2sh unlock script cannot use debug/panic opcode {}", *b)
                }
                _ => {}
            }
        }
        Ok(())
    }

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
        Self::verify_unlocks_bytecode(&cap, &unlocks)?;
        if unlocks.len() + lockbox.len() > cap.one_function_size {
            return errf!("p2sh code too long")
        }
        // verify combined code to avoid inconsistencies between segments
        let mut combined = Vec::with_capacity(unlocks.len() + lockbox.len());
        combined.extend_from_slice(&unlocks);
        combined.extend_from_slice(&lockbox);
        convert_and_check(&cap, ctb, &combined)?;
        // ok 
        let merkel = self.get_merkel()?.to_vec();
        let libs = self.adrlibs.serialize();
        let combined_len = combined.len();
        let mut stuff = Vec::with_capacity(
            merkel.len() + libs.len() + combined_len
        );
        stuff.extend_from_slice(&merkel);
        stuff.extend_from_slice(&libs);
        stuff.append(&mut combined);
        Ok(UnlockScript{ stuff })
    }

    fn get_merkel(&self) -> Ret<Address> {
        Ok(Self::calc_scriptmh_from_lockbox(&self.adrlibs, &self.lockbox, &self.merkels)?.address)
    }
    
}
