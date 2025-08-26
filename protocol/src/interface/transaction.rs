
pub trait TxExec {
    fn execute(&self, _: &mut dyn Context) -> Rerr { never!() }
}


pub trait TransactionRead : Serialize + TxExec + Send + Sync + DynClone { 
    fn ty(&self) -> u8 { never!() }


    fn hash(&self) -> Hash { never!() }
    fn hash_with_fee(&self) -> Hash { never!() }

    fn main(&self) -> Address { never!() }
    fn addrs(&self) -> Vec<Address> { never!() }

    fn timestamp(&self) -> &Timestamp { never!() }

    fn fee(&self) -> &Amount { never!() }
    fn fee_pay(&self) -> Amount { never!() }
    fn fee_got(&self) -> Amount { never!() }
    fn fee_extend(&self) -> Ret<(u16, Amount)> { never!() }
    
    fn message(&self) -> &Fixed16 { never!() }
    fn reward(&self) -> &Amount { never!() }

    fn action_count(&self) -> &Uint2 { never!() }
    fn actions(&self) -> &Vec<Box<dyn Action>> { never!() }
    fn signs(&self) -> &Vec<Sign> { never!() }
    
    fn req_sign(&self) -> Ret<HashSet<Address>> { never!() }
    fn verify_signature(&self) -> Rerr { never!() }

    fn fee_purity(&self) -> u64 { never!() }

    // burn_90_percent_fee
    fn burn_90(&self) -> bool {
        for act in self.actions() {
            if act.burn_90() {
                return true
            }
        }
        // not burn
        false
    }
}   


pub trait Transaction : TransactionRead + Field + Send + Sync {
    fn as_read(&self) -> &dyn TransactionRead;

    fn set_fee(&mut self, _: Amount) { never!(); }
    fn set_nonce(&mut self, _: Hash) { never!(); }

    fn fill_sign(&mut self,_: &Account) -> Ret<Sign> { never!() }
    fn push_sign(&mut self,_: Sign) -> Rerr { never!() }
    fn push_action(&mut self, _: Box<dyn Action>) -> Rerr { never!() }

}


clone_trait_object!(TransactionRead);
clone_trait_object!(Transaction);


