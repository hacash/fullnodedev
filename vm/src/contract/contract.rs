

#[derive(Clone)]
pub struct Contract {
    argv: BytesW2,
    ctrt: ContractSto
}


impl Contract {

    pub fn serialize(&self) -> Vec<u8> {
        self.ctrt.serialize()
    }
    
    pub fn new() -> Self {
        Self {
            argv: BytesW2::new(),
            ctrt: ContractSto::new()
        }
    }

    pub fn lib(mut self, a: Address) -> Self {
        let adr = ContractAddress::from_addr(a).unwrap();
        self.ctrt.librarys.push(adr).unwrap();
        self
    }

    pub fn inh(mut self, a: Address) -> Self {
        let adr = ContractAddress::from_addr(a).unwrap();
        self.ctrt.inherits.push(adr).unwrap();
        self
    }

    pub fn syst(mut self, a: Abst) -> Self {
        self.ctrt.abstcalls.push(a.func).unwrap();
        self
    }

    pub fn func(mut self, a: Func) -> Self {
        self.ctrt.userfuncs.push(a.func).unwrap();
        self
    }

    pub fn argv(mut self, a: Vec<u8>) -> Self {
        self.argv = BytesW2::from(a).unwrap();
        self
    }

    pub fn into_sto(self) -> ContractSto {
        self.ctrt
    }

    pub fn into_edit(self, expect_revision: u16) -> ContractEdit {
        let mut edit = ContractEdit::new();
        edit.expect_revision = Uint2::from(expect_revision);
        edit.inherits_add = self.ctrt.inherits;
        edit.librarys_add = self.ctrt.librarys;
        edit.abstcalls = self.ctrt.abstcalls;
        edit.userfuncs = self.ctrt.userfuncs;
        edit
    }

    fn estimate_protocol_cost(txfee: &Amount, payload_bytes: usize, charged_bytes: usize) -> Amount {
        if charged_bytes == 0 {
            return Amount::zero()
        }
        // CLI print helper: conservative estimate to reduce "protocol_cost too small" rejection.
        // Real validation still uses on-chain tx.fee_purity().
        const TX_SIZE_ESTIMATE_BASE_BYTES: u128 = 220;
        const SAFETY_NUM: u128 = 120; // +20% headroom
        const SAFETY_DEN: u128 = 100;
        let fee238 = txfee.to_238_u128().unwrap_or(0);
        let tx_size_est = TX_SIZE_ESTIMATE_BASE_BYTES.saturating_add(payload_bytes as u128);
        let mut fee_purity = fee238 / tx_size_est.max(1);
        if fee238 > 0 && fee_purity == 0 {
            fee_purity = 1;
        }
        let need = fee_purity
            .saturating_mul(charged_bytes as u128)
            .saturating_mul(CONTRACT_STORE_PERM_PERIODS as u128);
        let need = need
            .saturating_mul(SAFETY_NUM)
            .saturating_add(SAFETY_DEN - 1)
            / SAFETY_DEN;
        Amount::coin_u128(need, UNIT_238)
    }

    pub fn testnet_deploy_print_by_nonce(&self, fee: &str, nonce: u32) {
        let txfee = Amount::from(fee).unwrap();
        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(nonce);
        act.contract = self.ctrt.clone();
        act.construct_argv = self.argv.clone();
        let bytes = act.contract.size();
        act.protocol_cost = Self::estimate_protocol_cost(&txfee, bytes, bytes);
        // print
        curl_trs_2(vec![Box::new(act)], fee);
    } 

    pub fn testnet_deploy_print(&self, fee: &str) {
        self.testnet_deploy_print_by_nonce(fee, 0)
    } 

    pub fn testnet_update_print(&self, cadr: Address, fee: &str, expect_revision: u16) {
        let txfee = Amount::from(fee).unwrap();
        let mut act = ContractUpdate::new();
        act.edit = self.clone().into_edit(expect_revision);
        act.address = cadr;
        // On-chain update fee is charged by positive delta bytes only; helper uses edit size as estimate.
        let bytes = act.edit.size();
        act.protocol_cost = Self::estimate_protocol_cost(&txfee, bytes, bytes);
        // print
        curl_trs_2(vec![Box::new(act)], fee);
    } 


}
