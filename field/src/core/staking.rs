/// HIP-25 staking constants and on-chain types (v3 economics).

pub const STAKING_FEE_SHARE_PERCENT: u64 = 10;
pub const STAKING_POOL_SWEEP_BLOCKS: u64 = 1008;
pub const COOLDOWN_BLOCKS: u64 = 864;
pub const MIN_STAKE_BLOCKS: u64 = 25714;

pub const STAKING_EVENT_STAKED: Uint1 = Uint1::from(1);
pub const STAKING_EVENT_UNSTAKE_REQUESTED: Uint1 = Uint1::from(2);
pub const STAKING_EVENT_UNSTAKED: Uint1 = Uint1::from(3);
pub const STAKING_EVENT_REWARD_DISTRIBUTED: Uint1 = Uint1::from(4);
pub const STAKING_EVENT_POOL_SWEPT: Uint1 = Uint1::from(5);

#[inline]
pub fn diamond_status_allows_transfer(status: &Uint1) -> bool {
    *status == DIAMOND_STATUS_NORMAL
}

#[inline]
pub fn diamond_status_allows_inscription(status: &Uint1) -> bool {
    *status == DIAMOND_STATUS_NORMAL
}

#[inline]
pub fn diamond_status_is_staking_locked(status: &Uint1) -> bool {
    *status == DIAMOND_STATUS_STAKED || *status == DIAMOND_STATUS_STAKING_COOLDOWN
}

combi_struct! { StakingGlobal,
    total_staked_shares : Uint5
    global_reward_index : Uint8
    reward_pool_zhu     : Uint8
    paused              : Uint1
    unlock_queue_head   : Uint5
    unlock_queue_tail   : Uint5
    activation_height   : BlockHeight
    event_log_tail      : Uint5
    cumulative_deposit_zhu : Uint8
    cumulative_paid_zhu    : Uint8
    cumulative_pool_burned_zhu : Uint8
    zero_staker_blocks     : Uint5
}

impl StakingGlobal {
    pub fn is_paused(&self) -> bool {
        self.paused.uint() != 0
    }

    pub fn is_active_at(&self, height: u64, configured_activation: u64) -> bool {
        let act = self.activation_height.uint();
        let threshold = if act > 0 { act } else { configured_activation };
        threshold > 0 && height >= threshold
    }
}

combi_struct! { StakingRecord,
    stake_height   : BlockHeight
    unlock_height  : BlockHeight
    reward_index   : Uint8
    pending_reward : Amount
}

impl StakingRecord {
    pub fn is_active_stake(&self) -> bool {
        self.unlock_height.uint() == 0
    }
}

combi_struct! { StakingUnlockEntry,
    unlock_height : BlockHeight
    diamond       : DiamondName
    staker        : Address
    reward        : Amount
}

combi_struct! { StakingEvent,
    kind          : Uint1
    height        : BlockHeight
    diamond       : DiamondName
    staker        : Address
    unlock_height : BlockHeight
    reward        : Amount
    shares        : Uint5
}