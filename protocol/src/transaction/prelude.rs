pub const PRELUDE_TX_TYPE: u8 = 0;

#[inline]
pub fn is_prelude_tx_type(ty: u8) -> bool {
    ty == PRELUDE_TX_TYPE
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultPreludeTx {
    pub ty: Uint1,
    pub address: Address,
    pub reward: Amount,
    pub message: Fixed16,
    pub miner_nonce: Hash,
}

impl Default for DefaultPreludeTx {
    fn default() -> Self {
        Self {
            ty: Uint1::from(Self::TYPE),
            address: Address::default(),
            reward: Amount::small_mei(1),
            message: Fixed16::default(),
            miner_nonce: Hash::default(),
        }
    }
}

impl Parse for DefaultPreludeTx {
    fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
        let mut mv = 0;
        mv += self.ty.parse_from(buf)?;
        mv += self.address.parse_from(buf)?;
        mv += self.reward.parse_from(buf)?;
        mv += self.message.parse_from(buf)?;
        mv += self.miner_nonce.parse_from(buf)?;
        Ok(mv)
    }
}

impl Serialize for DefaultPreludeTx {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.ty.serialize_to(out);
        self.address.serialize_to(out);
        self.reward.serialize_to(out);
        self.message.serialize_to(out);
        self.miner_nonce.serialize_to(out);
    }

    fn size(&self) -> usize {
        self.ty.size()
            + self.address.size()
            + self.reward.size()
            + self.message.size()
            + self.miner_nonce.size()
    }
}

impl Field for DefaultPreludeTx {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for DefaultPreludeTx {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"ty\":{},\"address\":{},\"reward\":{},\"message\":{},\"miner_nonce\":{}}}",
            self.ty.to_json_fmt(fmt),
            self.address.to_json_fmt(fmt),
            self.reward.to_json_fmt(fmt),
            self.message.to_json_fmt(fmt),
            self.miner_nonce.to_json_fmt(fmt),
        )
    }
}

impl FromJSON for DefaultPreludeTx {
    fn from_json(&mut self, json_str: &str) -> Ret<()> {
        let pairs = json_split_object(json_str)?;
        for (k, v) in pairs {
            if k == "ty" {
                self.ty.from_json(v)?;
            } else if k == "address" {
                self.address.from_json(v)?;
            } else if k == "reward" {
                self.reward.from_json(v)?;
            } else if k == "message" {
                self.message.from_json(v)?;
            } else if k == "miner_nonce" {
                self.miner_nonce.from_json(v)?;
            }
        }
        Ok(())
    }
}

impl TransactionRead for DefaultPreludeTx {
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
        errf!("cannot verify signature on prelude tx")
    }
}

impl Transaction for DefaultPreludeTx {
    fn as_read(&self) -> &dyn TransactionRead {
        self
    }

    fn set_nonce(&mut self, nonce: Hash) {
        self.set_mining_nonce(nonce)
    }

    fn set_mining_nonce(&mut self, nonce: Hash) {
        self.miner_nonce = nonce
    }
}

impl TxExec for DefaultPreludeTx {
    fn execute(&self, ctx: &mut dyn Context) -> Rerr {
        operate::hac_add(ctx, &self.address, &self.reward)?;
        Ok(())
    }
}

impl DefaultPreludeTx {
    pub const TYPE: u8 = PRELUDE_TX_TYPE;
}
