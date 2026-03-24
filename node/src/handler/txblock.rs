

async fn handle_new_tx(this: Arc<MsgHandler>, peer: Option<Arc<Peer>>, body: Vec<u8>) {
    // println!("1111111 handle_txblock_arrive Tx, peer={} len={}", peer.nick(), body.clone().len());
    let engcnf = this.engine.config();
    let minter = this.engine.minter();
    // parse
    let Ok(txpkg) = protocol::transaction::build_tx_package(body) else {
        return // parse tx error
    };
    // tx hash with fee
    let hxfe = txpkg.tx().hash_with_fee();
    let (already, knowkey) = check_know(&this.knows, &hxfe, peer.clone());
    if already {
        return  // alreay know it
    }
    // println!("- devtest p2p recv new tx: {}, {}", txpkg.objc.hash().half(), hxfe.nonce());
    // check fee purity
    if txpkg.fpur() < engcnf.lowest_fee_purity {
        return // tx fee purity too low to broadcast
    }
    if txpkg.data().len() > engcnf.max_tx_size {
        return // tx size overflow
    }
    let txdatas = txpkg.data().to_vec();
    let txpr = txpkg.tx_read();
    // try execute and check tx
    if let Err(..) = this.engine.try_execute_tx(txpr) {
        return // tx execute fail
    }
    if let Err(..) = minter.tx_submit(this.engine.as_read(), &txpkg) {
        return // tx check fail
    }
    // add to tx pool
    let _ = this.txpool.insert_by(txpkg, &|tx|minter.tx_pool_group(tx));
    // broadcast
    let p2p = this.p2pmng.lock().unwrap();
    let p2p = p2p.as_ref().unwrap();
    p2p.broadcast_message(0/*not delay*/, knowkey, MSG_TX_SUBMIT, txdatas);
}


async fn handle_new_block(this: Arc<MsgHandler>, peer: Option<Arc<Peer>>, body: Vec<u8>) {
    let eng = this.engine.clone();
    let engcnf = eng.config();
    if body.len() > engcnf.max_block_size {
        return // block size overflow
    }
    let mut blkhead = protocol::block::BlockIntro::default();
    if let Err(..) = blkhead.parse(&body) {
        return // parse error
    }
    let blkhei = blkhead.height().uint();
    let blkhx = blkhead.hash();
    let (already, knowkey) = check_know(&this.knows, &blkhx, peer.clone());
    if already {
        return  // alreay know it
    }
    let mintckr = eng.minter();
    let sto = eng.store();
    if let Some(ret) = mintckr.blk_found_pending(&blkhead, &body, sto.as_ref()) {
        match ret {
            RetBlkFound::Reject => return,
            RetBlkFound::PendingCached => {
                let p2p = this.p2pmng.lock().unwrap();
                let p2p = p2p.as_ref().unwrap();
                p2p.broadcast_message(0/*not delay*/, knowkey, MSG_BLOCK_DISCOVER, body);
                return
            }
            RetBlkFound::Normal => {}
        }
    }
    let status = sto.status();
    let root_hei = status.root_height.uint();
    let heispan = engcnf.unstable_block;
    let latest = eng.latest_block();
    let lathei = latest.height().uint();
    if blkhei <= root_hei {
        return // height too late
    }
    if blkhei > lathei + 1 {
        // req sync
        if let Some(ref pr) = peer {
            send_req_block_hash_msg(pr.clone(), (heispan+1) as u8, lathei).await;
        }
        if lathei + heispan + 1 < blkhei {
            println!(
                "[P2P] ignore future block height={} root_height={} local_head={} store_height={} during history sync",
                blkhei,
                root_hei,
                lathei,
                status.last_height.uint(),
            );
        }
        return // not broadcast
    }
    if let Err(..) = mintckr.blk_found(&blkhead, &body, sto.as_ref()) {
        return
    }
    let blkpkg = protocol::block::build_block_package(body.clone());
    if let Err(..) = blkpkg {
        return // parse error
    }
    let mut blkp = blkpkg.unwrap();
    blkp.set_origin( BlkOrigin::Discover );
    // do insert  ◆ ◇ ⊙ ■ □ △ ▽ ❏ ❐ ❑ ❒  ▐ ░ ▒ ▓ ▔ ▕ ■ □ ▢ ▣ ▤ ▥ ▦ ▧ ▨ ▩ ▪ ▫
    let hxstrt = &blkhx.as_bytes()[4..12];
    let hxtail = &blkhx.as_bytes()[30..];
    let txs = blkp.block().transaction_count().uint() - 1;
    let _blkts = &timeshow(blkp.block().timestamp().uint())[14..];
    let isrlk = this.inserting.lock().unwrap();
    print!("❏ block {} …{}…{} txs{:2} insert at {} ",
        blkhei, hex::encode(hxstrt), hex::encode(hxtail), txs, &ctshow()[11..]);
    let engptr = eng.clone();
    let txpool = this.txpool.clone();
    may_show_miner_detail(engcnf, &blkp);
    let thsx = blkp.block().transaction_hash_list(false); // hash no fee
    if let Err(e) = engptr.discover(blkp) {
        println!("Error: {}", e);
        if e == LOW_BID_CACHE_FULL_ERR {
            drop(isrlk);
            return;
        }
    } else {
        println!("ok.");
        mintckr.tx_pool_refresh(engptr.as_ref().as_read(), txpool.as_ref(), thsx, blkhei);
    }
    drop(isrlk); // close lock
    // broadcast new block
    let p2p = this.p2pmng.lock().unwrap();
    let p2p = p2p.as_ref().unwrap();
    p2p.broadcast_message(0/*not delay*/, knowkey, MSG_BLOCK_DISCOVER, body);
}



// return already know
fn check_know(mine: &Knowledge, hxkey: &Hash, peer: Option<Arc<Peer>>) -> (bool, KnowKey) {
    let knowkey: [u8; KNOWLEDGE_SIZE] = hxkey.clone().into_array();
    if let Some(ref pr) = peer {
        pr.knows.add(knowkey.clone());
    }
    if mine.check(&knowkey) {
        return (true, knowkey) // alreay know it
    }
    mine.add(knowkey.clone());
    (false, knowkey)
}


fn may_show_miner_detail(engcnf: &EngineConf, blkp: &BlkPkg) {
    if !engcnf.show_miner_name {
        return
    }
    // devtest start
    let Ok(cbtx) = blkp.block().coinbase_transaction() else {
        return
    };
    let adrt = cbtx.main().to_readable().drain(..9).collect::<String>();
    print!("miner: {}...<{}> ", adrt, cbtx.message().to_readable_left());
    // devtest end
}
