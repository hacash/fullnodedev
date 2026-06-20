/// HIP-25 staking operations (v3 economics, CoreState).

const DIAMOND_MINT_ACTION_KIND: u16 = 4;

fn staking_accrued_amount(global_index: &Uint8, snapshot: &Uint8) -> Ret<Amount> {
    let zhu = global_index.uint().saturating_sub(snapshot.uint());
    if zhu == 0 {
        return Ok(Amount::default());
    }
    Ok(Amount::zhu(zhu))
}

pub fn staking_is_active_at_height(state: &CoreState<'_>, height: u64, configured_activation: u64) -> bool {
    state.get_staking_global().is_active_at(height, configured_activation)
}

/// True when tx fee miner share should fund the staking pool (DiamondMint bid only).
pub fn staking_tx_qualifies_for_mint_fee_redirect(tx: &dyn TransactionRead) -> bool {
    tx.actions()
        .iter()
        .any(|a| a.kind() == DIAMOND_MINT_ACTION_KIND && a.extra9())
}

pub fn staking_deposit_fee(state: &mut CoreState<'_>, fee_zhu: u64) {
    if fee_zhu == 0 {
        return;
    }
    let mut global = state.get_staking_global();
    global.reward_pool_zhu = Uint8::from(global.reward_pool_zhu.uint() + fee_zhu);
    global.cumulative_deposit_zhu =
        Uint8::from(global.cumulative_deposit_zhu.uint() + fee_zhu);
    state.set_staking_global(&global);
}

pub fn staking_deposit_mint_miner_share(state: &mut CoreState<'_>, fee: &Amount) {
    if !fee.is_positive() {
        return;
    }
    let zhu = fee.to_zhu_u64().unwrap_or(0);
    staking_deposit_fee(state, zhu);
}

fn staking_push_event(state: &mut CoreState<'_>, event: &StakingEvent) {
    let mut global = state.get_staking_global();
    let id = global.event_log_tail.uint();
    state.staking_event_set(&Uint5::from(id), event);
    global.event_log_tail = Uint5::from(id + 1);
    state.set_staking_global(&global);
}

pub fn staking_sweep_idle_pool(state: &mut CoreState<'_>, height: u64) -> XRet<()> {
    let mut global = state.get_staking_global();
    let shares = global.total_staked_shares.uint();
    let pool = global.reward_pool_zhu.uint();
    if shares > 0 || pool == 0 {
        global.zero_staker_blocks = Uint5::from(0);
        state.set_staking_global(&global);
        return Ok(());
    }
    let idle = global.zero_staker_blocks.uint() + 1;
    global.zero_staker_blocks = Uint5::from(idle);
    if idle < STAKING_POOL_SWEEP_BLOCKS {
        state.set_staking_global(&global);
        return Ok(());
    }
    global.reward_pool_zhu = Uint8::from(0);
    global.zero_staker_blocks = Uint5::from(0);
    global.cumulative_pool_burned_zhu =
        Uint8::from(global.cumulative_pool_burned_zhu.uint() + pool);
    state.set_staking_global(&global);
    let burn_amt = Amount::zhu(pool);
    with_total_count(state, |ttcount| {
        total_add_amount_238(
            &mut ttcount.hacd_bid_burn_238,
            &burn_amt,
            "hacd_bid_burn_238",
        )
    })?;
    staking_push_event(
        state,
        &StakingEvent {
            kind: STAKING_EVENT_POOL_SWEPT,
            height: BlockHeight::from(height),
            diamond: DiamondName::default(),
            staker: Address::default(),
            unlock_height: BlockHeight::from(0),
            reward: burn_amt,
            shares: Uint5::from(0),
        },
    );
    Ok(())
}

pub fn staking_distribute_rewards(state: &mut CoreState<'_>, height: u64) -> XRet<()> {
    let mut global = state.get_staking_global();
    let shares = global.total_staked_shares.uint();
    let pool = global.reward_pool_zhu.uint();
    if shares == 0 || pool == 0 {
        return Ok(());
    }
    let increment = pool / shares;
    if increment > 0 {
        global.global_reward_index =
            Uint8::from(global.global_reward_index.uint() + increment);
    }
    let distributed = increment * shares;
    if distributed >= pool {
        global.reward_pool_zhu = Uint8::from(0);
    } else {
        global.reward_pool_zhu = Uint8::from(pool - distributed);
    }
    state.set_staking_global(&global);
    if distributed > 0 {
        staking_push_event(
            state,
            &StakingEvent {
                kind: STAKING_EVENT_REWARD_DISTRIBUTED,
                height: BlockHeight::from(height),
                diamond: DiamondName::default(),
                staker: Address::default(),
                unlock_height: BlockHeight::from(0),
                reward: Amount::zhu(distributed),
                shares: Uint5::from(shares),
            },
        );
    }
    Ok(())
}

fn staking_enqueue_unlock(state: &mut CoreState<'_>, entry: &StakingUnlockEntry) -> XRet<()> {
    let mut global = state.get_staking_global();
    let id = global.unlock_queue_tail.uint();
    state.staking_unlock_set(&Uint5::from(id), entry);
    global.unlock_queue_tail = Uint5::from(id + 1);
    state.set_staking_global(&global);
    Ok(())
}

fn staking_finalize_unlock(state: &mut CoreState<'_>, entry: &StakingUnlockEntry) -> XRet<()> {
    let dianame = &entry.diamond;
    let mut diaitem = match state.diamond(dianame) {
        Some(d) => d,
        None => {
            return xerrf!("diamond {} not found", dianame.to_readable());
        }
    };
    if diaitem.status != DIAMOND_STATUS_STAKING_COOLDOWN {
        return xerrf!(
            "diamond {} unlock failed: expected cooldown status",
            dianame.to_readable()
        );
    }
    diaitem.status = DIAMOND_STATUS_NORMAL;
    state.diamond_set(dianame, &diaitem);
    state.staking_record_del(dianame);
    staking_push_event(
        state,
        &StakingEvent {
            kind: STAKING_EVENT_UNSTAKED,
            height: entry.unlock_height.clone(),
            diamond: entry.diamond.clone(),
            staker: entry.staker.clone(),
            unlock_height: entry.unlock_height.clone(),
            reward: entry.reward.clone(),
            shares: Uint5::from(0),
        },
    );
    Ok(())
}

pub fn staking_process_unlock_queue(state: &mut CoreState<'_>, height: u64) -> XRet<()> {
    let mut pending: Vec<(Uint5, StakingUnlockEntry)> = Vec::new();
    let mut global = state.get_staking_global();
    let mut head = global.unlock_queue_head.uint();
    let tail = global.unlock_queue_tail.uint();
    while head < tail {
        let key = Uint5::from(head);
        let entry = match state.staking_unlock(&key) {
            Some(e) => e,
            None => {
                return xerrf!(
                    "staking unlock queue corrupted: missing entry {} (head {} tail {})",
                    head,
                    head,
                    tail
                );
            }
        };
        if entry.unlock_height.uint() > height {
            break;
        }
        pending.push((key, entry));
        head += 1;
    }
    global.unlock_queue_head = Uint5::from(head);
    state.set_staking_global(&global);

    for (key, entry) in pending {
        let reward = entry.reward.clone();
        let staker = entry.staker.clone();
        if reward.is_positive() {
            hac_add_balance(state, &staker, &reward)?;
            let mut global = state.get_staking_global();
            let paid = reward.to_zhu_u64().unwrap_or(0);
            global.cumulative_paid_zhu =
                Uint8::from(global.cumulative_paid_zhu.uint() + paid);
            state.set_staking_global(&global);
        }
        staking_finalize_unlock(state, &entry)?;
        state.staking_unlock_del(&key);
    }
    Ok(())
}

pub fn staking_on_block_close(
    state: &mut CoreState<'_>,
    height: u64,
    configured_activation: u64,
) -> XRet<()> {
    if !staking_is_active_at_height(state, height, configured_activation) {
        return Ok(());
    }
    staking_sweep_idle_pool(state, height)?;
    staking_distribute_rewards(state, height)?;
    staking_process_unlock_queue(state, height)?;
    Ok(())
}

pub fn check_diamond_stakeable(
    state: &CoreState<'_>,
    staker: &Address,
    hacd_name: &DiamondName,
) -> XRet<DiamondSto> {
    let diaitem = match state.diamond(hacd_name) {
        Some(d) => d,
        None => {
            return xerrf!("diamond {} not found", hacd_name.to_readable());
        }
    };
    if !diamond_status_allows_transfer(&diaitem.status) {
        let msg = if diamond_status_is_staking_locked(&diaitem.status) {
            format!(
                "diamond {} cannot be staked while status is {} (staking locked)",
                hacd_name.to_readable(),
                diaitem.status.uint()
            )
        } else {
            format!(
                "diamond {} cannot be staked while status is {}",
                hacd_name.to_readable(),
                diaitem.status.uint()
            )
        };
        return xerrf!("{}", msg);
    }
    if *staker != diaitem.address {
        return xerrf!(
            "diamond {} not belong to address {}",
            hacd_name.to_readable(),
            staker
        );
    }
    Ok(diaitem)
}

pub fn staking_set_paused(state: &mut CoreState<'_>, paused: bool) {
    let mut global = state.get_staking_global();
    global.paused = Uint1::from(if paused { 1 } else { 0 });
    state.set_staking_global(&global);
}

pub fn staking_apply_stake(
    state: &mut CoreState<'_>,
    staker: &Address,
    diamonds: &DiamondNameListMax200,
    height: u64,
    configured_activation: u64,
) -> XRet<()> {
    diamonds.check()?;
    let mut global = state.get_staking_global();
    if !global.is_active_at(height, configured_activation) {
        return xerrf!("HACD staking is not active at height {}", height);
    }
    if global.is_paused() {
        return xerrf!("HACD staking is paused");
    }
    let reward_index = global.global_reward_index.clone();

    for dianame in diamonds.clone().into_iter() {
        let mut diaitem = check_diamond_stakeable(state, staker, &dianame)?;
        diaitem.status = DIAMOND_STATUS_STAKED;
        state.diamond_set(&dianame, &diaitem);

        let record = StakingRecord {
            stake_height: BlockHeight::from(height),
            unlock_height: BlockHeight::from(0),
            reward_index: reward_index.clone(),
            pending_reward: Amount::default(),
        };
        state.staking_record_set(&dianame, &record);
        global.total_staked_shares = Uint5::from(global.total_staked_shares.uint() + 1);

        staking_push_event(
            state,
            &StakingEvent {
                kind: STAKING_EVENT_STAKED,
                height: BlockHeight::from(height),
                diamond: dianame.clone(),
                staker: staker.clone(),
                unlock_height: BlockHeight::from(0),
                reward: Amount::default(),
                shares: global.total_staked_shares.clone(),
            },
        );
    }

    state.set_staking_global(&global);
    Ok(())
}

pub fn staking_apply_unstake(
    state: &mut CoreState<'_>,
    staker: &Address,
    diamonds: &DiamondNameListMax200,
    height: u64,
) -> XRet<()> {
    diamonds.check()?;
    let global = state.get_staking_global();
    let reward_index = global.global_reward_index.clone();

    for dianame in diamonds.clone().into_iter() {
        let mut diaitem = match state.diamond(&dianame) {
            Some(d) => d,
            None => {
                return xerrf!("diamond {} not found", dianame.to_readable());
            }
        };
        if diaitem.status != DIAMOND_STATUS_STAKED {
            return xerrf!("diamond {} is not staked", dianame.to_readable());
        }
        if *staker != diaitem.address {
            return xerrf!(
                "diamond {} not belong to staker {}",
                dianame.to_readable(),
                staker
            );
        }

        let record = match state.staking_record(&dianame) {
            Some(r) => r,
            None => {
                return xerrf!(
                    "staking record for {} not found",
                    dianame.to_readable()
                );
            }
        };
        let stake_height = record.stake_height.uint();
        if height < stake_height + MIN_STAKE_BLOCKS {
            return xerrf!(
                "diamond {} must remain staked for at least {} blocks",
                dianame.to_readable(),
                MIN_STAKE_BLOCKS
            );
        }

        let reward = staking_accrued_amount(&reward_index, &record.reward_index)?;

        diaitem.status = DIAMOND_STATUS_STAKING_COOLDOWN;
        state.diamond_set(&dianame, &diaitem);

        let unlock_height = height + COOLDOWN_BLOCKS;
        let cooldown_record = StakingRecord {
            stake_height: record.stake_height.clone(),
            unlock_height: BlockHeight::from(unlock_height),
            reward_index: reward_index.clone(),
            pending_reward: reward.clone(),
        };
        state.staking_record_set(&dianame, &cooldown_record);

        let mut global = state.get_staking_global();
        global.total_staked_shares =
            Uint5::from(global.total_staked_shares.uint().saturating_sub(1));
        state.set_staking_global(&global);

        let entry = StakingUnlockEntry {
            unlock_height: BlockHeight::from(unlock_height),
            diamond: dianame.clone(),
            staker: staker.clone(),
            reward: reward.clone(),
        };
        staking_enqueue_unlock(state, &entry)?;

        staking_push_event(
            state,
            &StakingEvent {
                kind: STAKING_EVENT_UNSTAKE_REQUESTED,
                height: BlockHeight::from(height),
                diamond: dianame.clone(),
                staker: staker.clone(),
                unlock_height: BlockHeight::from(unlock_height),
                reward: entry.reward.clone(),
                shares: Uint5::from(0),
            },
        );
    }

    Ok(())
}