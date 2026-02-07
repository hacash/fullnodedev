
fn impl_packing_next_block(this: &HacashMinter, engine: &dyn EngineRead, txpool: &dyn TxPool) -> Box<dyn Any> {
        
    let engcnf = engine.config();

    let mtcnf = this.cnf;
    let oldblk = engine.latest_block();
    
    let prevhash = oldblk.hash();
    let mut newdifn = oldblk.difficulty().clone();
    if *newdifn == 0 {
        newdifn = Uint4::from(LOWEST_DIFFICULTY);
    }
    let nexthei = oldblk.height().uint() + 1;
    // update difficulty number
    if nexthei % mtcnf.difficulty_adjust_blocks == 0 {
        let sto = engine.store();
        let (difn, ..) = this.next_difficulty(oldblk.as_read(), sto.as_ref());
        newdifn = Uint4::from(difn);
    }
    // create coinbase tx
    let cbtx = create_coinbase_tx(nexthei, engcnf.miner_message.clone(), 
        engcnf.miner_reward_address.clone());
    // create block v1
    let mut intro = BlockIntro {
        head: BlockHead {
            version           : Uint1::from(1),
            height            : BlockHeight::from(nexthei),
            timestamp         : Timestamp::from(curtimes()),
            prevhash          : prevhash,
            mrklroot          : Hash::default(),
            transaction_count : Uint4::default()
        },
        meta: BlockMeta {
            nonce         : Uint4::default(), 
            difficulty    : newdifn, 
            witness_stage : Fixed2::default()
        }
    };
    /* debug test
    // intro.head.timestamp = Timestamp::from(1723385108);
    // intro.meta.nonce = Uint4::from(4191621845);
    // cbtx.message = StringTrim16::must(&hex::decode("62616f6b756169000000000000006f56").unwrap());
    // test end*/
    // trs with cbtx
    let mut trslen: usize = 1;
    // MerkleRoot consensus uses tx hash WITH fee.
    // Must keep consistent with chain-side verification (transaction_hash_list(true)).
    let mut trshxs: Vec<Hash> = vec![cbtx.hash_with_fee()];
    // trs
    let mut transactions = DynVecTransaction::default();
    transactions.push(Box::new(cbtx.clone())).unwrap();
    
    append_valid_tx_pick_from_txpool( nexthei, 
        &mut trslen, &mut trshxs, &mut transactions, 
        cbtx.size(),
        engine, txpool,
    );

    // set mrkl & trs count
    intro.head.mrklroot = calculate_mrklroot(&trshxs);
    intro.head.transaction_count = Uint4::from(trslen as u32);

    // ok
    let block = BlockV1{ intro, transactions };

    Box::new(block)

}



pub fn create_coinbase_tx(hei: u64, msg: Fixed16, adr: Address) -> TransactionCoinbase {
    let rwdamt = genesis::block_reward(hei);
    TransactionCoinbase {
        ty      : Uint1::from(0), // ccoinbase type = 0
        address : adr,
        reward  : rwdamt,
        message : msg,
        extend  : CoinbaseExtend::must(CoinbaseExtendDataV1 {
            miner_nonce: Hash::default(),
            witness_count: Uint1::from(0),
        })
    }
}




/*
    park txs to block
*/
fn append_valid_tx_pick_from_txpool(pending_hei: u64, trslen: &mut usize, trshxs: &mut Vec<Hash>, 
    trs: &mut DynVecTransaction, base_tx_size: usize, engine: &dyn EngineRead, txpool: &dyn TxPool,
) {
    let engcnf = engine.config();
    let txmaxn = engcnf.max_block_txs;
    let txmaxsz = engcnf.max_block_size;
    let mut allfee = Amount::zero();
    let mut txallsz: usize = base_tx_size;
    let txallsz = &mut txallsz;
    let mut invalidtxhxs = Vec::new();

    let mut sub_state = engine.fork_sub_state();

    macro_rules! ok_push_one_tx {
        ($a: expr, $txsz: expr) => {
            if trs.push($a.objc.clone()).is_err() {
                return false
            }
            trshxs.push($a.objc.as_ref().as_read().hash_with_fee());
            *trslen += 1;
            *txallsz += $txsz;
        }
    }

    macro_rules! check_pick_one_tx {
        ($a: expr) => {
            let txr = $a.objc.as_ref().as_read();
            if let Err(..) = txr.verify_signature() {
                invalidtxhxs.push(txr.hash());
                return true // sign fail, ignore, next
            }
            if let Err(..) = engine.try_execute_tx_by(txr, pending_hei, &mut sub_state) {
                invalidtxhxs.push(txr.hash());
                return true // execute fail, ignore, next
            };
            let Ok(nf) = allfee.add_mode_u64(&$a.objc.fee_got()) else {
                invalidtxhxs.push(txr.hash());
                return true; // fee size err, ignore, next
            };
            allfee = nf;
        }

    }

    // pick one diamond mint tx
    if pending_hei % 5 == 0 {
        let mut pick_dmint = |a: &TxPkg| {
            let txsz = a.data.len();
            if txsz + *txallsz > txmaxsz {
                return true // try next one
            }
            if *trslen >= txmaxn {
                return false // end
            }
            // check tx
            check_pick_one_tx!(a);
            // ok push
            ok_push_one_tx!(a, txsz);
            false // end
        };
        txpool.iter_at(TXGID_DIAMINT, &mut pick_dmint).unwrap();
    }

    // pick normal tx
    let mut pick_normal_tx = |a: &TxPkg| {
        let txsz = a.data.len();
        if *trslen >= txmaxn {
            return false // end, num enough
        }
        if txsz + *txallsz > txmaxsz {
            return true // skip oversize tx and continue
        }
        // check tx
        check_pick_one_tx!(a);
        ok_push_one_tx!(a, txsz);
        true // next
    };
    txpool.iter_at(TXGID_NORMAL, &mut pick_normal_tx).unwrap();

    // delete invalid txs
    if invalidtxhxs.len() > 0 {
        let _ = txpool.drain(&invalidtxhxs);
    }
    // ok
}



/********************************************/


#[cfg(test)]
mod tests {
    use super::*;

    use basis::config::EngineConf;

    use std::collections::HashMap;

    struct DummyStore;
    impl Store for DummyStore {
        fn status(&self) -> ChainStatus { ChainStatus::default() }
        fn save_block_data(&self, _: &Hash, _: &Vec<u8>) {}
        fn save_block_hash(&self, _: &BlockHeight, _: &Hash) {}
        fn save_block_hash_path(&self, _: &dyn MemDB) {}
        fn save_batch(&self, _: &dyn MemDB) {}
        fn block_data(&self, _: &Hash) -> Option<Vec<u8>> { None }
        fn block_hash(&self, _: &BlockHeight) -> Option<Hash> { None }
        fn block_data_by_height(&self, _: &BlockHeight) -> Option<(Hash, Vec<u8>)> { None }
    }

    struct DummyState;
    impl State for DummyState {
        fn fork_sub(&self, _: Weak<Box<dyn State>>) -> Box<dyn State> { Box::new(DummyState) }
        fn merge_sub(&mut self, _: Box<dyn State>) {}
        fn detach(&mut self) {}
        fn clone_state(&self) -> Box<dyn State> { Box::new(DummyState) }
        fn as_mem(&self) -> &MemMap {
            static EMPTY: std::sync::LazyLock<MemMap> = std::sync::LazyLock::new(HashMap::new);
            &EMPTY
        }
        fn disk(&self) -> Arc<dyn DiskDB> { never!() }
        fn write_to_disk(&self) {}
        fn get(&self, _: Vec<u8>) -> Option<Vec<u8>> { None }
        fn set(&mut self, _: Vec<u8>, _: Vec<u8>) {}
        fn del(&mut self, _: Vec<u8>) {}
    }

    struct OneTxPool {
        tx: TxPkg,
    }
    impl TxPool for OneTxPool {
        fn iter_at(&self, gid: usize, each: &mut dyn FnMut(&TxPkg) -> bool) -> Rerr {
            match gid {
                TXGID_NORMAL => {
                    let _ = each(&self.tx);
                }
                TXGID_DIAMINT => {}
                _ => {}
            }
            Ok(())
        }
        fn drain(&self, _: &[Hash]) -> Ret<Vec<TxPkg>> { Ok(vec![]) }
        fn print(&self) -> String { s!("OneTxPool") }
    }

    struct TestEngine {
        cnf: EngineConf,
        latest: Arc<dyn Block>,
        store: Arc<dyn Store>,
    }

    impl EngineRead for TestEngine {
        fn config(&self) -> &EngineConf { &self.cnf }
        fn latest_block(&self) -> Arc<dyn Block> { self.latest.clone() }
        fn store(&self) -> Arc<dyn Store> { self.store.clone() }
        fn fork_sub_state(&self) -> Box<dyn State> { Box::new(DummyState) }
        fn try_execute_tx_by(&self, _: &dyn TransactionRead, _: u64, _: &mut Box<dyn State>) -> Rerr { Ok(()) }
    }

    #[test]
    fn packing_merkle_root_uses_hash_with_fee() {
        // HacashMinter::create() constructs the genesis block, which validates against a
        // hard-coded mainnet hash. That requires the global block hasher to be configured.
        protocol::setup::block_hasher(x16rs::block_hash);

        // Keep test independent from genesis init checks (which require the global block hasher
        // to be configured by the binary).
        let prev_blk = {
            let cbtx = TransactionCoinbase {
                ty: Uint1::from(0),
                address: Address::default(),
                reward: Amount::small_mei(1),
                message: Fixed16::default(),
                extend: CoinbaseExtend::default(),
            };
            let intro = BlockIntro {
                head: BlockHead {
                    version: Uint1::from(1),
                    height: BlockHeight::from(0),
                    timestamp: Timestamp::from(1549250700),
                    prevhash: Hash::default(),
                    mrklroot: Hash::default(),
                    transaction_count: Uint4::from(1),
                },
                meta: BlockMeta {
                    nonce: Uint4::default(),
                    difficulty: Uint4::from(0),
                    witness_stage: Fixed2::default(),
                },
            };

            let mut txs = DynVecTransaction::default();
            txs.push(Box::new(cbtx)).unwrap();

            let mut blk = BlockV1 { intro, transactions: txs };
            blk.update_mrklroot();
            blk
        };

        // Build a tx where hash() != hash_with_fee() (fee is non-zero).
        let acc = Account::create_by_password("merkle-test").unwrap();
        let main = Address::from(*acc.address());

        let mut tx = TransactionType2::new_by(main, Amount::small(1, 248), curtimes());
        tx.fill_sign(&acc).unwrap();
        let txp = TxPkg::create(Box::new(tx));

        let tp = OneTxPool { tx: txp };

        let mut cnf = EngineConf {
            max_block_txs: 1000,
            max_block_size: 1024 * 1024,
            max_tx_size: 1024 * 16,
            max_tx_actions: 200,
            chain_id: 0,
            unstable_block: 4,
            fast_sync: false,
            sync_maxh: 0,
            data_dir: "/tmp".to_string(),
            block_data_dir: std::path::PathBuf::from("/tmp/hacash_test_block"),
            state_data_dir: std::path::PathBuf::from("/tmp/hacash_test_state"),
            blogs_data_dir: std::path::PathBuf::from("/tmp/hacash_test_logs"),
            show_miner_name: false,
            vmlogs_enable: false,
            vmlogs_open_height: 0,
            vmlogs_can_delete: false,
            dev_count_switch: 0,
            diamond_form: true,
            recent_blocks: false,
            average_fee_purity: false,
            lowest_fee_purity: 0,
            miner_enable: false,
            miner_reward_address: Address::default(),
            miner_message: Fixed16::default(),
            dmer_enable: false,
            dmer_reward_address: Address::default(),
            dmer_bid_account: Account::create_by_password("123456").unwrap(),
            dmer_bid_min: Amount::small_mei(1),
            dmer_bid_max: Amount::small_mei(31),
            dmer_bid_step: Amount::small(5, 247),
            txpool_maxs: Vec::default(),
        };
        // Make sure we set miner message/reward (pack uses these).
        cnf.miner_message = Fixed16::default();
        cnf.miner_reward_address = Address::default();

        let engine = TestEngine {
            cnf,
            latest: Arc::new(prev_blk),
            store: Arc::new(DummyStore),
        };

        let minter = HacashMinter::create(&IniObj::default());
        let blk_any = minter.packing_next_block(&engine, &tp);
        let blk = *blk_any.downcast::<BlockV1>().unwrap();

        let want = calculate_mrklroot(&blk.transaction_hash_list(true));
        assert_eq!(*blk.mrklroot(), want);
    }
}




/********************************************/



fn impl_tx_pool_refresh(_this: &HacashMinter, eng: &dyn EngineRead, txpool: &dyn TxPool, txs: Vec<Hash>, blkhei: u64) {

    if blkhei % 15 == 0 {
        println!("{}.", txpool.print());
    }
    // drop all overdue diamond mint tx
    if blkhei % 5 == 0 {
        clean_invalid_diamond_mint_txs(eng, txpool, blkhei);
    }
    // drop all exist normal tx
    if txs.len() > 1 {
        let _ = txpool.drain(&txs[1..]); // over coinbase tx
    }
    // drop invalid normal
    if blkhei % 11 == 0 { // 1 hours
        clean_invalid_normal_txs(eng, txpool, blkhei);
    }
}


// clean_
fn clean_invalid_normal_txs(eng: &dyn EngineRead, txpool: &dyn TxPool, blkhei: u64) {
    let pdhei = blkhei + 1;
    let mut sub_state = eng.fork_sub_state();
    // already minted hacd number
    let _ = txpool.retain_at(TXGID_NORMAL, &mut |a: &TxPkg| {
        let txr = a.objc.as_read();
        if txr.verify_signature().is_err() {
            return false;
        }
        let exec = eng.try_execute_tx_by(txr, pdhei, &mut sub_state);
        exec.is_ok() // keep or delete 
    });
}


// clean_
fn clean_invalid_diamond_mint_txs(eng: &dyn EngineRead, txpool: &dyn TxPool, _blkhei: u64) {
    // already minted hacd number
    let sta = eng.state();
    let sta = sta.as_ref();
    let curdn = CoreStateRead::wrap(sta.as_ref()).get_latest_diamond().number.uint();
    let nextdn = curdn + 1;
    let _ = txpool.retain_at(TXGID_DIAMINT, &mut |a: &TxPkg| {
        // must be next diamond number, or delete
        nextdn == action::get_diamond_mint_number(a.objc.as_read())
    });
}
