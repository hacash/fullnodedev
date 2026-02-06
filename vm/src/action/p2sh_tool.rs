/*
    P2SH tooling helpers.

    Design goals:
    - Canonical: given the same set of (libs, lockbox) leaves, every implementation should
      derive the same scriptmh address. We achieve this by sorting leaves by their *leaf hash*
      (the same leaf commitment used by consensus).
    - Consensus-aligned: leaf/branch hashing rules are intentionally identical to
      `UnlockScriptProve::calc_scriptmh_from_lockbox` / `get_merkel()`.
    - Hard to misuse: APIs return intermediate hashes and generate `UnlockScriptProve` instances
      for a selected leaf, so wallet/SDK code does not need to hand-roll Merkle paths.
*/


/// A single P2SH leaf: `(adrlibs, lockbox)`.
///
/// Note: libs are part of the leaf commitment. If libs differ, even with identical lockbox
/// bytecode, the leaf hash (and therefore the final scriptmh address) will differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2shLeafSpec {
    pub adrlibs: ContractAddressW1,
    pub lockbox: BytesW2,
}

#[derive(Debug, Clone)]
pub struct P2shLeaf {
    pub spec: P2shLeafSpec,
    pub leaf_hash: Hash,
}

/// Merkle tree root result.
#[derive(Debug, Clone)]
pub struct P2shTreeCalc {
    pub root_sha3: Hash,
    pub payload20: [u8; 20],
    pub address: Address,
}

/// Canonical Merkle rule: if a level has an odd count, duplicate the last node.
#[derive(Debug, Clone, Copy, Default)]
pub enum MerkleRule {
    #[default]
    DuplicateLastWhenOdd,
}

/// A canonical P2SH Merkle tree (leaves sorted by leaf commitment hash).
#[derive(Debug, Clone)]
pub struct P2shMerkleTree {
    rule: MerkleRule,
    leaves: Vec<P2shLeaf>,       // canonical order
    levels: Vec<Vec<Hash>>,      // levels[0] = leaves hashes, levels.last() = [root]
    calc: P2shTreeCalc,
}

/// Tool "class" wrapper: contains the canonical construction algorithms and helpers.
pub struct P2shTool;

impl P2shTool {
    /// Build a canonical Merkle tree from raw leaf specs.
    ///
    /// Canonical ordering:
    /// - Compute each leaf commitment hash (same leaf commitment as consensus).
    /// - Sort leaves by `leaf_hash` ascending (bytewise).
    ///
    /// Safety:
    /// - Rejects duplicate `leaf_hash` to avoid ambiguous selection APIs.
    pub fn build_canonical_tree(mut specs: Vec<P2shLeafSpec>) -> Ret<P2shMerkleTree> {
        if specs.is_empty() {
            return errf!("p2sh tool: leaf specs cannot be empty")
        }
        let empty_path = MerkelStuffs::from_list(vec![])?;
        let mut leaves: Vec<P2shLeaf> = Vec::with_capacity(specs.len());
        for spec in specs.drain(..) {
            let calc = UnlockScriptProve::calc_scriptmh_from_lockbox(&spec.adrlibs, &spec.lockbox, &empty_path)?;
            let leaf_hash = calc.sha3_path[0].clone();
            leaves.push(P2shLeaf{ spec, leaf_hash });
        }
        leaves.sort_by(|a, b| a.leaf_hash.serialize().cmp(&b.leaf_hash.serialize()));
        for i in 1..leaves.len() {
            if leaves[i - 1].leaf_hash == leaves[i].leaf_hash {
                return errf!("p2sh tool: duplicate leaf hash {}", hex::encode(leaves[i].leaf_hash.serialize()))
            }
        }
        Self::build_tree_from_sorted_leaves(MerkleRule::DuplicateLastWhenOdd, leaves)
    }

    /// Convenience: build a canonical tree from a list of lockbox scripts that share the same libs.
    pub fn build_canonical_tree_shared_libs(adrlibs: ContractAddressW1, lockboxes: Vec<BytesW2>) -> Ret<P2shMerkleTree> {
        if lockboxes.is_empty() {
            return errf!("p2sh tool: lockbox list cannot be empty")
        }
        let specs: Vec<_> = lockboxes
            .into_iter()
            .map(|lockbox| P2shLeafSpec{ adrlibs: adrlibs.clone(), lockbox })
            .collect();
        Self::build_canonical_tree(specs)
    }

    fn build_tree_from_sorted_leaves(rule: MerkleRule, leaves: Vec<P2shLeaf>) -> Ret<P2shMerkleTree> {
        let mut levels: Vec<Vec<Hash>> = vec![];
        let mut cur: Vec<Hash> = leaves.iter().map(|l|l.leaf_hash.clone()).collect();
        levels.push(cur.clone());

        while cur.len() > 1 {
            let mut next: Vec<Hash> = Vec::with_capacity((cur.len() + 1) / 2);
            let mut i = 0usize;
            while i < cur.len() {
                let left = cur[i].clone();
                let right = match (i + 1).cmp(&cur.len()) {
                    std::cmp::Ordering::Less => cur[i + 1].clone(),
                    _ => match rule {
                        MerkleRule::DuplicateLastWhenOdd => cur[i].clone(),
                    },
                };
                let mut buf = Vec::with_capacity("p2sh_branch_".len() + 32 + 32);
                buf.extend_from_slice("p2sh_branch_".as_bytes());
                buf.extend_from_slice(&left.serialize());
                buf.extend_from_slice(&right.serialize());
                next.push(Hash::from(sha3(buf)));
                i += 2;
            }
            cur = next.clone();
            levels.push(next);
        }

        let root_sha3 = levels.last().unwrap()[0].clone();
        let payload20 = ripemd160(root_sha3);
        let address = Address::create_scriptmh(payload20);
        Ok(P2shMerkleTree{
            rule,
            leaves,
            levels,
            calc: P2shTreeCalc{ root_sha3, payload20, address },
        })
    }
}

impl P2shMerkleTree {
    pub fn address(&self) -> Address { self.calc.address }
    pub fn root_sha3(&self) -> Hash { self.calc.root_sha3.clone() }
    pub fn leaves(&self) -> &Vec<P2shLeaf> { &self.leaves }
    pub fn merkle_rule(&self) -> MerkleRule { self.rule }

    /// Return the Merkle proof path (siblings + posi) for the leaf at canonical index `idx`.
    ///
    /// `posi` semantics match consensus `get_merkel()`:
    /// - `posi==0`: sibling hash is on the LEFT
    /// - `posi==1`: sibling hash is on the RIGHT
    pub fn proof_for_index(&self, idx: usize) -> Ret<MerkelStuffs> {
        if idx >= self.leaves.len() {
            return errf!("p2sh tool: leaf index {} overflow (len={})", idx, self.leaves.len())
        }
        let mut path: Vec<PosiHash> = vec![];
        let mut i = idx;
        // levels[0] is leaf level, levels.last() is root level (len==1)
        for level in &self.levels[..self.levels.len() - 1] {
            let n = level.len();
            let (sib_idx, posi) = if i % 2 == 0 {
                let sib = if i + 1 < n { i + 1 } else { i };
                (sib, 1u8) // sibling on the right
            } else {
                (i - 1, 0u8) // sibling on the left
            };
            path.push(PosiHash{
                posi: Uint1::from(posi),
                hash: level[sib_idx].clone(),
            });
            i /= 2;
        }
        MerkelStuffs::from_list(path)
    }

    pub fn select_index_by_leaf_hash(&self, leaf_hash: &Hash) -> Ret<usize> {
        self.leaves
            .iter()
            .position(|l| &l.leaf_hash == leaf_hash)
            .ok_or_else(|| format!("p2sh tool: leaf hash {} not found", hex::encode(leaf_hash.serialize())))
    }

    pub fn select_index_by_spec(&self, adrlibs: &ContractAddressW1, lockbox: &BytesW2) -> Ret<usize> {
        self.leaves
            .iter()
            .position(|l| &l.spec.adrlibs == adrlibs && &l.spec.lockbox == lockbox)
            .ok_or_else(|| "p2sh tool: leaf (libs, lockbox) not found".to_owned())
    }

    /// Build an `UnlockScriptProve` action for the leaf at canonical index `idx`.
    ///
    /// Returns:
    /// - the final `scriptmh` address (should be used as `from` address)
    /// - the filled `UnlockScriptProve` action (ready to be included in tx)
    /// - the intermediate `ScriptmhCalc` (leaf->root path), for debugging/tooling
    ///
    /// This function does NOT validate bytecode (it does not know the current `SpaceCap`).
    /// The chain will validate in `UnlockScriptProve::execute`.
    pub fn build_unlock_script_prove_unchecked(&self, idx: usize, witness: BytesW2) -> Ret<(Address, UnlockScriptProve, ScriptmhCalc)> {
        let spec = self.leaves.get(idx).ok_or_else(|| format!("p2sh tool: leaf index {} overflow", idx))?.spec.clone();
        let merkels = self.proof_for_index(idx)?;
        let calc = UnlockScriptProve::calc_scriptmh_from_lockbox(&spec.adrlibs, &spec.lockbox, &merkels)?;
        if calc.address != self.calc.address {
            return errf!(
                "p2sh tool: proof derived address {} mismatch tree address {}",
                calc.address,
                self.calc.address
            )
        }
        let mut act = UnlockScriptProve::new();
        act.argvkey = witness;
        act.lockbox = spec.lockbox;
        act.adrlibs = spec.adrlibs;
        act.merkels = merkels;
        // `_marks_` keeps default zero (Fixed2::default), which passes the on-chain check.
        Ok((calc.address, act, calc))
    }

    /// Same as `build_unlock_script_prove_unchecked`, but performs local checks using
    /// the same rules as `UnlockScriptProve::get_stuff`.
    pub fn build_unlock_script_prove_checked(&self, block_height: u64, idx: usize, witness: BytesW2) -> Ret<(Address, UnlockScriptProve, ScriptmhCalc)> {
        let spec = self.leaves.get(idx).ok_or_else(|| format!("p2sh tool: leaf index {} overflow", idx))?.spec.clone();
        let cap = SpaceCap::new(block_height);
        let ctb = CodeType::Bytecode;
        // lockbox must be valid by itself
        convert_and_check(&cap, ctb, spec.lockbox.as_vec())?;
        // witness bytes must fit local constraints
        UnlockScriptProve::verify_witness_bytes(&cap, witness.as_vec())?;
        // ok, build action
        self.build_unlock_script_prove_unchecked(idx, witness)
    }
}


#[cfg(test)]
mod p2sh_tool_test {
    use super::*;

    fn dummy_lockbox(byte: u8) -> BytesW2 {
        // simplest valid bytecode must end with END/RET/...
        BytesW2::from(vec![Bytecode::PU8 as u8, byte, Bytecode::END as u8]).unwrap()
    }

    #[test]
    fn canonical_tree_is_order_independent() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let s1 = P2shLeafSpec{ adrlibs: libs.clone(), lockbox: dummy_lockbox(1) };
        let s2 = P2shLeafSpec{ adrlibs: libs.clone(), lockbox: dummy_lockbox(2) };
        let t1 = P2shTool::build_canonical_tree(vec![s1.clone(), s2.clone()]).unwrap();
        let t2 = P2shTool::build_canonical_tree(vec![s2, s1]).unwrap();
        assert_eq!(t1.address(), t2.address());
        assert_eq!(t1.root_sha3(), t2.root_sha3());
    }

    #[test]
    fn proof_derives_tree_address() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let s1 = P2shLeafSpec{ adrlibs: libs.clone(), lockbox: dummy_lockbox(7) };
        let s2 = P2shLeafSpec{ adrlibs: libs.clone(), lockbox: dummy_lockbox(9) };
        let tree = P2shTool::build_canonical_tree(vec![s1, s2]).unwrap();
        // pick leaf 0 in canonical order
        let proof = tree.proof_for_index(0).unwrap();
        let spec = &tree.leaves()[0].spec;
        let calc = UnlockScriptProve::calc_scriptmh_from_lockbox(&spec.adrlibs, &spec.lockbox, &proof).unwrap();
        assert_eq!(calc.address, tree.address());
    }
}
