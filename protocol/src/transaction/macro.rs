macro_rules! transaction_define {
    ($class:ident, $tyid:expr) => {
        field::combi_struct! { $class,
            ty         : Uint1
            timestamp  : Timestamp
            addrlist   : AddrOrList
            fee        : Amount
            actions    : DynListActionW2
            signs      : SignW2
            gas_max    : Uint1
            ano_mark   : Fixed1
        }

        impl TransactionRead for $class {
            fn hash(&self) -> Hash {
                self.hash_ex(vec![]) // no fee field
            }

            fn hash_with_fee(&self) -> Hash {
                self.hash_ex(self.fee.serialize()) // with fee
            }

            fn ty(&self) -> u8 {
                *self.ty
            }

            fn main(&self) -> Address {
                self.addrs()[0] // must
            }

            fn addrs(&self) -> Vec<Address> {
                self.addrlist.to_list() // must
            }

            fn fee(&self) -> &Amount {
                &self.fee
            }

            fn timestamp(&self) -> &Timestamp {
                &self.timestamp
            }

            fn action_count(&self) -> usize {
                self.actions.length()
            }

            fn actions(&self) -> &Vec<Box<dyn Action>> {
                self.actions.as_list()
            }

            fn signs(&self) -> &Vec<Sign> {
                self.signs.as_list()
            }

            // burn_90_percent_fee
            fn burn_90(&self) -> bool {
                // By design only serialized top-level tx actions decide tx-wide burn_90; nested VM/runtime actions do not upgrade the whole transaction.
                self.actions().iter().any(|a| a.burn_90())
            }

            fn fee_pay(&self) -> Amount {
                self.fee().clone()
            }
            // fee_miner_received
            fn fee_got(&self) -> Amount {
                let mut gfee = self.fee().clone();
                if self.burn_90() && gfee.unit() > 1 {
                    gfee = gfee.unit_sub(1).unwrap(); // burn 90
                }
                gfee
            }

            fn fee_extend(&self) -> Ret<u8> {
                // Only type3+ participates in gas_max fee extension; type1/2 must be 0.
                if $tyid < TransactionType3::TYPE {
                    return Ok(0);
                }
                Ok(*self.gas_max)
            }

            fn raw_gas_max(&self) -> u8 {
                *self.gas_max
            }

            fn raw_ano_mark(&self) -> u8 {
                self.ano_mark[0]
            }

            fn req_sign(&self) -> Ret<HashSet<Address>> {
                let addrs = &self.addrs();
                let mut adrsets = HashSet::from([self.main()]);
                for act in self.actions() {
                    for ptr in act.req_sign() {
                        let adr = ptr.real(addrs)?;
                        // Only PRIVAKEY addresses can provide signatures.
                        // Non-privakey addresses (contract/scriptmh) must be authorized by VM hooks instead.
                        if adr.is_privakey() {
                            adrsets.insert(adr);
                        }
                    }
                }
                Ok(adrsets)
            }

            fn verify_signature(&self) -> Rerr {
                verify_tx_signature(self)
            }

            // fee_purity: fee rate per byte in unit-238, used for miner tx ordering.
            fn fee_purity(&self) -> u64 {
                let txsz = self.size() as u64;
                if txsz == 0 {
                    return 0;
                }
                let fee238 = self.fee_got().to_238_u64().unwrap_or(0);
                fee238 / txsz
            }
        }

        impl Transaction for $class {
            fn as_read(&self) -> &dyn TransactionRead {
                self
            }

            fn set_fee(&mut self, fee: Amount) {
                self.fee = fee;
            }

            fn fill_sign(&mut self, acc: &Account) -> Ret<Sign> {
                let mut fhx = self.hash();
                // Historical compatibility keeps legacy Type1 main signing on hash_with_fee even though consensus verification still uses hash().
                if acc.address() == self.main().as_bytes() {
                    fhx = self.hash_with_fee();
                }
                // do sign
                // let apbk = acc.public_key().serialize_compressed();
                let signobj = Sign::create_by(acc, &fhx); /*{
                                                              publickey: Fixed33::from( apbk ),
                                                              signature: Fixed64::from( acc.do_sign(&fhx) ),
                                                          };*/
                // insert
                self.insert_sign(signobj.clone())?;
                Ok(signobj)
            }

            fn push_sign(&mut self, signobj: Sign) -> Rerr {
                self.insert_sign(signobj)
            }

            fn push_action(&mut self, act: Box<dyn Action>) -> Rerr {
                self.actions.push(act)
            }
        }

        impl TxExec for $class {
            fn execute(&self, ctx: &mut dyn TxDriverContext) -> Rerr {
                do_tx_execute(self, ctx)
            }
        }

        impl $class {
            pub const TYPE: u8 = $tyid;

            pub fn new_by(addr: Address, fee: Amount, ts: u64) -> $class {
                $class {
                    ty: Uint1::from($tyid),
                    timestamp: Timestamp::from(ts),
                    addrlist: AddrOrList::from_addr(addr),
                    fee: fee,
                    actions: DynListActionW2::default(),
                    signs: SignW2::default(),
                    gas_max: Uint1::default(),
                    ano_mark: Fixed1::default(),
                }
            }

            fn hash_ex(&self, adfe: Vec<u8>) -> Hash {
                let mut stuff = vec![
                    self.ty.serialize(),
                    self.timestamp.serialize(),
                    self.addrlist.serialize(),
                    adfe, /* self.fee.serialize()*/
                    self.actions.serialize(),
                ]
                .concat();
                // ignore signs data
                if $tyid >= TransactionType3::TYPE {
                    stuff.append(&mut self.gas_max.serialize());
                    stuff.append(&mut self.ano_mark.serialize());
                }
                let hx = sys::calculate_hash(stuff);
                Hash::must(&hx[..])
            }

            fn insert_sign(&mut self, signobj: Sign) -> Rerr {
                let plen = self.signs.length();
                if plen >= u16::MAX as usize - 1 {
                    return errf!("too many sign objects");
                }
                let curaddr = Address::from(Account::get_address_by_public_key(*signobj.publickey));
                // insert
                let apbk = signobj.publickey.as_ref();
                let mut istid = usize::MAX;
                let sglist = self.signs.as_list();
                for i in 0..plen {
                    let pbk = sglist[i].publickey.as_bytes();
                    if apbk == pbk {
                        istid = i;
                        break;
                    }
                }
                // append
                if istid == usize::MAX {
                    self.signs.push(signobj)?;
                } else {
                    // replace
                    self.signs.as_mut()[istid] = signobj;
                }
                if let Ok(yes) = verify_target_signature(&curaddr, self) {
                    if yes {
                        return Ok(());
                    }
                }
                // verify error
                errf!("address {} signature verification failed", curaddr)
            }
        }
    };
}

/*
*
*/
fn do_tx_execute(tx: &dyn Transaction, ctx: &mut dyn TxDriverContext) -> Rerr {
    const TXTY1: u8 = TransactionType1::TYPE;
    const _TXTY2: u8 = TransactionType2::TYPE;
    const TXTY3: u8 = TransactionType3::TYPE;
    let env = ctx.env();
    let blkhei = env.block.height;
    crate::upgrade::check_gated_tx(blkhei, tx.ty())?;
    let not_fast_sync = !env.chain.fast_sync;
    let hx = tx.hash();
    let main = tx.main();
    let fee = tx.fee();
    let has_ast_control = tx
        .actions()
        .iter()
        .any(|a| crate::action::is_ast_container_action(a.as_ref()));
    check_tx_action_ast_tree_depth(tx.actions())?;
    let mut state = CoreState::wrap(ctx.state());
    // may fast_sync
    if not_fast_sync {
        // Tx-level action-set checks (length bounds) are centralized here.
        analyze_tx_action_set(tx.actions())?;
        // main check
        if !main.is_privakey() {
            return errf!("tx fee address version must be PRIVAKEY type.");
        }
        for adr in tx.addrs() {
            adr.check_version()?; // check all address version
        }
        let mty = tx.ty();
        // check BlockHeight more than 20w trs.Fee.Size() must less than 6 bytes.
        if blkhei > 20_0000 {
            fee.check_6_long().map_err(|_| {
                "tx fee size cannot exceed 6 bytes when block height above 200,000".to_string()
            })?;
        }
        if blkhei > 33033 && mty <= TXTY1 {
            // last is 33019
            return errf!("Type 1 transactions have been deprecated after height 33,033");
        }
        // Defense-in-depth: tx.execute may be called by mempool/runtime checks directly.
        // Keep signature verification on the execution path to avoid unsigned tx bypass.
        tx.verify_signature()?;
        // check tx exist
        if let Some(exhei) = state.tx_exist(&hx) {
            // have tx !!!
            // handle hacash block chain bug start
            let bugtx =
                Hash::from_hex(b"f22deb27dd2893397c2bc203ddc9bc9034e455fe630d8ee310e8b5ecc6dc5628")
                    .unwrap();
            if *exhei != 63448 || hx != bugtx {
                return errf!("tx {} already exists in height {}", hx, *exhei);
            }
            // pass the BUG
        }
    }
    if tx.ty() < TXTY3 && has_ast_control {
        return errf!(
            "tx type {} cannot include AST control-flow actions; requires at least type {}",
            tx.ty(),
            TXTY3
        );
    }
    let txty = tx.ty();
    if txty < TXTY3 {
        let gas_max = tx.raw_gas_max();
        if gas_max != 0 {
            return errf!("tx type {} gas_max must be zero", txty);
        }
        let ano_mark = tx.raw_ano_mark();
        if ano_mark != 0 {
            return errf!("tx type {} ano_mark must be zero", txty);
        }
    }
    // set tx exist mark
    state.tx_exist_set(&hx, &BlockHeight::from(blkhei));
    let gas_max_byte = tx.fee_extend().unwrap_or(0);

    let gas_enabled = gas_max_byte > 0;
    if gas_enabled {
        let (budget, gas_rate) = crate::context::tx_gas_params_from_byte(gas_max_byte)?;
        ctx.gas_init_tx(budget, gas_rate)?;
    }
    // execute actions
        let exec_res: Rerr = (|| {
            for action in tx.actions() {
                ctx.exec_from_set(ExecFrom::Top);
                // Top-level tx loop intentionally ignores return gas; only AST nesting and VM ACTION consume it.
                let (_ret_gas, _retv) = action.execute(ctx)?;
            }
            Ok(())
    })();

    let mut settle_res: Rerr = Ok(());
    if gas_enabled {
        settle_res = ctx.gas_refund();
    }
    settle_res?;
    exec_res?;

    super::tex::do_settlement(ctx)?;

    // spend fee
    operate::hac_sub(ctx, &main, fee)?;
    let fee_got = tx.fee_got();
    let burn_fee = fee.sub_mode_u128(&fee_got)?;
    if burn_fee.is_positive() {
        // TotalCount stores burn stats in unit238 (u64). Converting here floors any amount
        // below one unit238; this is an intentional precision/storage trade-off.
        let burn_238 = burn_fee.to_238_u64()?;
        if burn_238 > 0 {
            let mut state = CoreState::wrap(ctx.state());
            let mut ttcount = state.get_total_count();
            let next_burn = (*ttcount.tx_fee_burn90_238)
                .checked_add(burn_238)
                .ok_or_else(|| "tx_fee_burn90_238 overflow".to_string())?;
            ttcount.tx_fee_burn90_238 = Uint8::from(next_burn);
            state.set_total_count(&ttcount);
        }
    }
    // ok finish
    Ok(())
}
