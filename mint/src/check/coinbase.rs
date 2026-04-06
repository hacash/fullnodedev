use protocol::operate;

combi_struct! { CoinbaseExtendDataV1,
    miner_nonce   : Hash
    witness_count : Uint1
}

combi_optional! { CoinbaseExtend,
    extend: CoinbaseExtendDataV1
}

combi_struct! { TransactionCoinbase,
    ty      : Uint1
    address : Address
    reward  : Amount
    message : Fixed16
    extend  : CoinbaseExtend
}

impl TransactionRead for TransactionCoinbase {
    fn author(&self) -> Option<Address> {
        Some(self.address.clone())
    }

    fn block_message(&self) -> Option<&Fixed16> {
        Some(&self.message)
    }

    fn block_reward(&self) -> Option<&Amount> {
        Some(&self.reward)
    }

    fn fee_receiver(&self) -> Option<Address> {
        Some(self.address.clone())
    }

    fn hash(&self) -> Hash {
        let stuff = self.serialize();
        let hx = sys::calculate_hash(stuff);
        Hash::must(&hx[..])
    }

    fn hash_with_fee(&self) -> Hash {
        self.hash()
    }

    fn fee_pay(&self) -> Amount {
        Amount::zero()
    }

    fn fee_got(&self) -> Amount {
        Amount::zero()
    }

    fn gas_max_byte(&self) -> Option<u8> {
        None
    }

    fn ty(&self) -> u8 {
        *self.ty
    }

    fn main(&self) -> Address {
        self.address.clone()
    }

    fn addrs(&self) -> Vec<Address> {
        vec![self.main()]
    }

    fn reward(&self) -> &Amount {
        &self.reward
    }

    fn message(&self) -> &Fixed16 {
        &self.message
    }

    fn verify_signature(&self) -> Rerr {
        errf!("cannot verify signature on coinbase tx")
    }
}

impl Transaction for TransactionCoinbase {
    fn as_read(&self) -> &dyn TransactionRead {
        self
    }

    fn set_nonce(&mut self, nonce: Hash) {
        self.set_mining_nonce(nonce)
    }

    fn set_mining_nonce(&mut self, nonce: Hash) {
        match &mut self.extend.extend {
            Some(d) => d.miner_nonce = nonce,
            _ => (),
        };
    }
}

impl TxExec for TransactionCoinbase {
    fn execute(&self, ctx: &mut dyn Context) -> Rerr {
        operate::hac_add(ctx, &self.address, &self.reward)?;
        Ok(())
    }
}

impl TransactionCoinbase {
    pub const TYPE: u8 = 0;
}



fn verify_coinbase(height: u64, cbtx: &dyn TransactionRead) -> Rerr {
    let got = cbtx.reward();
    let need = genesis::block_reward(height);
    if need != *got {
        return errf!("block coinbase reward expected {} but got {}", need, got)
    }
    // ok    
    Ok(())
}
