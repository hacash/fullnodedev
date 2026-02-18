/// HIP-22 unified inscription cooldown: 200 blocks
const INSCRIPTION_COOLDOWN_BLOCKS: u64 = 200;
const INSCRIPTION_CONTENT_MAX_BYTES: usize = 64;
const INSCRIPTION_READABLE_TYPE_MAX: u8 = 100;
pub const INSCRIPTION_MAX_PER_DIAMOND: usize = 200;
const APPEND_FREE_MAX_INSCRIPTIONS: usize = 10;
const APPEND_TIER1_MAX_INSCRIPTIONS: usize = 40;
const APPEND_TIER2_MAX_INSCRIPTIONS: usize = 100;

#[inline]
fn check_inscription_content(engraved_type: u8, content: &BytesW1) -> Rerr {
    let insc_len = content.length();
    if insc_len == 0 {
        return errf!("engraved content cannot be empty");
    }
    if insc_len > INSCRIPTION_CONTENT_MAX_BYTES {
        return errf!(
            "engraved content size cannot over {} bytes",
            INSCRIPTION_CONTENT_MAX_BYTES
        );
    }
    if engraved_type <= INSCRIPTION_READABLE_TYPE_MAX {
        if !check_readable_string(content) {
            return errf!("engraved content must readable string");
        }
    }
    Ok(())
}

#[inline]
fn check_inscription_owner_privakey(owner: &Address, diamond: &DiamondName) -> Rerr {
    if !owner.is_privakey() {
        return errf!(
            "diamond {} owner {} must be privakey address",
            diamond.to_readable(),
            owner
        );
    }
    Ok(())
}

#[inline]
fn check_diamond_status_for_inscription(
    state: &mut CoreState,
    owner: &Address,
    diamond: &DiamondName,
) -> Ret<DiamondSto> {
    check_inscription_owner_privakey(owner, diamond)?;
    let diasto = check_diamond_status(state, owner, diamond)?;
    Ok(diasto)
}

/// Load diamond, verify Normal status and PRIVAKEY owner, return (DiamondSto, owner).
/// Does NOT require a pre-known owner — used by Move where owners are discovered on-chain.
#[inline]
fn load_diamond_for_inscription(
    state: &mut CoreState,
    diamond: &DiamondName,
) -> Ret<DiamondSto> {
    let diasto = must_have!(
        format!("diamond status {}", diamond.to_readable()),
        state.diamond(diamond)
    );
    check_inscription_owner_privakey(&diasto.address, diamond)?;
    if diasto.status != DIAMOND_STATUS_NORMAL {
        return errf!(
            "diamond {} has been mortgaged and cannot operate inscription",
            diamond.to_readable()
        );
    }
    Ok(diasto)
}

#[inline]
fn check_inscription_cooldown(
    prev_engraved_height: u64,
    pending_height: u64,
    diamond: &DiamondName,
) -> Rerr {
    let next_height = prev_engraved_height.saturating_add(INSCRIPTION_COOLDOWN_BLOCKS);
    if next_height > pending_height {
        return errf!(
            "HACD {} inscription cooldown not met, need {} blocks",
            diamond.to_readable(),
            INSCRIPTION_COOLDOWN_BLOCKS
        );
    }
    Ok(())
}

/*
*
*/
action_define! { DiamondInscription, 32,
    ActLv::Top, // level
    true, // burn 90 fee
    [], // need sign
    {
        diamonds         : DiamondNameListMax200
        protocol_cost    : Amount
        engraved_type    : Uint1
        engraved_content : BytesW1
    },
    (self, {
        let a = self;
        let dia_num = a.diamonds.length();
        let cost_str = a.protocol_cost.to_fin_string();
        let ins_str = a.engraved_content.to_readable_or_hex();
        let mut desc = format!("Inscript {} HACD ({}) with \"{}\"",
            dia_num, a.diamonds.splitstr(), ins_str);
        if a.protocol_cost.is_positive() {
            desc += &format!(" cost {} HAC fee", cost_str);
        }
        desc
    }),
    (self, ctx, _gas {
        diamond_inscription(self, ctx)
    })
}

fn diamond_inscription(this: &DiamondInscription, ctx: &mut dyn Context) -> Ret<Vec<u8>> {
    let env = ctx.env().clone();
    let main_addr = env.tx.main;
    let pfee = &this.protocol_cost;
    if pfee.is_negative() {
        return errf!("protocol fee cannot be negative");
    }
    // check
    this.diamonds.check()?;
    if pfee.size() > 4 {
        return errf!("protocol fee amount size cannot over 4 bytes");
    }
    ctx.check_sign(&main_addr)?;
    // check inscription content
    check_inscription_content(*this.engraved_type, &this.engraved_content)?;
    // cost
    let mut ttcost = Amount::zero();
    let pdhei = env.block.height;
    // do
    let mut state = CoreState::wrap(ctx.state());
    for dia in this.diamonds.as_list() {
        let cc = engraved_one_diamond(pdhei, &mut state, &main_addr, &dia, &this.engraved_content)?;
        ttcost = ttcost.add_mode_u64(&cc)?;
    }
    // check cost
    if pfee < &ttcost {
        return errf!(
            "diamond inscription cost error need {:?} but got {:?}",
            ttcost,
            pfee
        );
    }
    // change count
    let mut ttcount = state.get_total_count();
    let engraved_total = (*ttcount.diamond_engraved)
        .checked_add(this.diamonds.length() as u64)
        .ok_or_else(|| "diamond_engraved overflow".to_string())?;
    ttcount.diamond_engraved = Uint8::from(engraved_total);
    let pfee_zhu = pfee.to_zhu_u64()?;
    let burn_total = (*ttcount.diamond_insc_burn_zhu)
        .checked_add(pfee_zhu)
        .ok_or_else(|| "diamond_insc_burn_zhu overflow".to_string())?;
    ttcount.diamond_insc_burn_zhu = Uint8::from(burn_total);
    state.set_total_count(&ttcount);
    // sub main addr balance
    if pfee.is_positive() {
        hac_sub(ctx, &main_addr, &pfee)?;
    }
    // ok
    Ok(vec![])
}

/************************************ */

action_define! { DiamondInscriptionClear, 33,
    ActLv::Top, // level
    true, // burn 90 fee
    [], // need sign
    {
        diamonds      : DiamondNameListMax200
        protocol_cost : Amount
    },
    (self, {
        let a = self;
        let dia_num = a.diamonds.length();
        let cost_str = a.protocol_cost.to_fin_string();
        format!(
            "Clean inscript {} HACD ({}) cost {} HAC fee",
            dia_num, a.diamonds.splitstr(), cost_str
        )
    }),
    (self, ctx, _gas {
        diamond_inscription_clean(self, ctx)
    })
}

fn diamond_inscription_clean(
    this: &DiamondInscriptionClear,
    ctx: &mut dyn Context,
) -> Ret<Vec<u8>> {
    let env = ctx.env().clone();
    let main_addr = env.tx.main;
    let pfee = &this.protocol_cost;
    if pfee.is_negative() {
        return errf!("protocol cost cannot be negative");
    }
    // check
    this.diamonds.check()?;
    if pfee.size() > 4 {
        return errf!("protocol cost amount size cannot over 4 bytes");
    }
    ctx.check_sign(&main_addr)?;
    // cost
    let mut ttcost = Amount::zero();
    let pdhei = env.block.height;
    // do
    let mut state = CoreState::wrap(ctx.state());
    for dia in this.diamonds.as_list() {
        let cc = engraved_clean_one_diamond(pdhei, &mut state, &main_addr, &dia)?;
        ttcost = ttcost.add_mode_u64(&cc)?;
    }
    // check cost
    if pfee < &ttcost {
        return errf!(
            "diamond inscription cost error need {:?} but got {:?}",
            ttcost,
            pfee
        );
    }
    // change count and sub hac
    if pfee.is_positive() {
        let mut ttcount = state.get_total_count();
        let pfee_zhu = pfee.to_zhu_u64()?;
        let burn_total = (*ttcount.diamond_insc_burn_zhu)
            .checked_add(pfee_zhu)
            .ok_or_else(|| "diamond_insc_burn_zhu overflow".to_string())?;
        ttcount.diamond_insc_burn_zhu = Uint8::from(burn_total);
        state.set_total_count(&ttcount);
        // sub main addr balance
        hac_sub(ctx, &main_addr, &pfee)?;
    }
    // finish
    Ok(vec![])
}

/************************************** */

/*
* HIP-22: Upgrade HACD Inscriptions
* Transfer / Per-entry Delete / Single-entry Update
*/

action_define! { DiamondInscriptionMove, 34,
    ActLv::Ast, // level
    true, // urn fee
    [], // need sign
    {
        from_diamond    : DiamondName
        to_diamond      : DiamondName
        index           : Uint1
        protocol_cost   : Amount
    },
    (self, {
        let a = self;
        let mut desc = format!("Move inscription #{} from HACD {} to HACD {}",
            *a.index, a.from_diamond.to_readable(),
            a.to_diamond.to_readable());
        if a.protocol_cost.is_positive() {
            desc += &format!(" cost {} HAC fee", a.protocol_cost.to_fin_string());
        }
        desc
    }),
    (self, ctx, _gas {
        #[cfg(not(feature = "hip22"))]
        if true { return errf!("HIP-22 not activated") }
        diamond_inscription_move(self, ctx)
    })
}

fn diamond_inscription_move(this: &DiamondInscriptionMove, ctx: &mut dyn Context) -> Ret<Vec<u8>> {
    let env = ctx.env().clone();
    let main_addr = env.tx.main;
    let pfee = &this.protocol_cost;
    if pfee.is_negative() {
        return errf!("protocol cost cannot be negative");
    }
    if pfee.size() > 4 {
        return errf!("protocol cost amount size cannot over 4 bytes");
    }
    let idx = *this.index as usize;
    let pdhei = env.block.height;
    if this.from_diamond == this.to_diamond {
        return errf!("source and target HACD cannot be the same");
    }
    // validate source and target diamonds
    let (mut from_sto, mut to_sto, from_owner, to_owner, move_cost) = {
        let mut state = CoreState::wrap(ctx.state());
        let from_sto = load_diamond_for_inscription(&mut state, &this.from_diamond)?;
        let from_owner = from_sto.address.clone();
        let from_insc_len = from_sto.inscripts.length();
        if from_insc_len == 0 {
            return errf!(
                "no inscriptions in source HACD {}",
                this.from_diamond.to_readable()
            );
        }
        if idx >= from_insc_len {
            return errf!(
                "inscription index {} out of range, HACD {} has {} inscriptions",
                idx,
                this.from_diamond.to_readable(),
                from_insc_len
            );
        }
        check_inscription_cooldown(*from_sto.prev_engraved_height, pdhei, &this.from_diamond)?;
        let to_sto = load_diamond_for_inscription(&mut state, &this.to_diamond)?;
        let to_owner = to_sto.address.clone();
        check_inscription_cooldown(*to_sto.prev_engraved_height, pdhei, &this.to_diamond)?;
        if to_sto.inscripts.length() >= INSCRIPTION_MAX_PER_DIAMOND {
            return errf!(
                "target HACD {} inscriptions full (max {})",
                this.to_diamond.to_readable(),
                INSCRIPTION_MAX_PER_DIAMOND
            );
        }
        let move_cost = {
            let to_len = to_sto.inscripts.length();
            let diaslt = must_have!(
                format!("diamond {}", this.to_diamond.to_readable()),
                state.diamond_smelt(&this.to_diamond)
            );
            calc_move_inscription_protocol_cost(to_len, *diaslt.average_bid_burn)
        };
        (from_sto, to_sto, from_owner, to_owner, move_cost)
    };
    // both owners must sign
    ctx.check_sign(&from_owner)?;
    ctx.check_sign(&to_owner)?;
    // check protocol cost
    if pfee < &move_cost {
        return errf!(
            "inscription move cost error need {:?} but got {:?}",
            move_cost,
            pfee
        );
    }
    // extract inscription from source
    let content = from_sto.inscripts.as_list()[idx].clone();
    from_sto.inscripts.drop(idx)?;
    from_sto.prev_engraved_height = BlockHeight::from(pdhei);
    let mut state = CoreState::wrap(ctx.state());
    state.diamond_set(&this.from_diamond, &from_sto);
    // push inscription to target
    to_sto.inscripts.push(content)?;
    to_sto.prev_engraved_height = BlockHeight::from(pdhei);
    state.diamond_set(&this.to_diamond, &to_sto);
    // burn protocol fee
    if pfee.is_positive() {
        let mut ttcount = state.get_total_count();
        let pfee_zhu = pfee.to_zhu_u64()?;
        let burn_total = (*ttcount.diamond_insc_burn_zhu)
            .checked_add(pfee_zhu)
            .ok_or_else(|| "diamond_insc_burn_zhu overflow".to_string())?;
        ttcount.diamond_insc_burn_zhu = Uint8::from(burn_total);
        state.set_total_count(&ttcount);
        hac_sub(ctx, &main_addr, pfee)?;
    }
    // ok
    Ok(vec![])
}

/************************************ */

action_define! { DiamondInscriptionDrop, 35,
    ActLv::Top, // level
    true, // burn 90 fee
    [], // need sign
    {
        diamond           : DiamondName
        index             : Uint1
        protocol_cost     : Amount
    },
    (self, {
        let a = self;
        format!("Drop inscription #{} from HACD {} cost {} HAC fee",
            *a.index, a.diamond.to_readable(), a.protocol_cost.to_fin_string())
    }),
    (self, ctx, _gas {
        #[cfg(not(feature = "hip22"))]
        if true { return errf!("HIP-22 not activated") }
        diamond_inscription_drop(self, ctx)
    })
}

fn diamond_inscription_drop(this: &DiamondInscriptionDrop, ctx: &mut dyn Context) -> Ret<Vec<u8>> {
    let env = ctx.env().clone();
    let main_addr = env.tx.main;
    let pfee = &this.protocol_cost;
    if pfee.is_negative() {
        return errf!("protocol cost cannot be negative");
    }
    if pfee.size() > 4 {
        return errf!("protocol cost amount size cannot over 4 bytes");
    }
    ctx.check_sign(&main_addr)?;
    let idx = *this.index as usize;
    let pdhei = env.block.height;
    // check diamond
    let mut state = CoreState::wrap(ctx.state());
    let mut diasto = check_diamond_status_for_inscription(&mut state, &main_addr, &this.diamond)?;
    let insc_len = diasto.inscripts.length();
    if insc_len == 0 {
        return errf!("no inscriptions in HACD {}", this.diamond.to_readable());
    }
    if idx >= insc_len {
        return errf!(
            "inscription index {} out of range, HACD {} has {} inscriptions",
            idx,
            this.diamond.to_readable(),
            insc_len
        );
    }
    // check cooldown
    check_inscription_cooldown(*diasto.prev_engraved_height, pdhei, &this.diamond)?;
    // cost: average_bid_burn / 50
    let diaslt = must_have!(
        format!("diamond {}", this.diamond.to_readable()),
        state.diamond_smelt(&this.diamond)
    );
    let cost = calc_drop_inscription_protocol_cost(*diaslt.average_bid_burn);
    if pfee < &cost {
        return errf!(
            "inscription drop cost error need {:?} but got {:?}",
            cost,
            pfee
        );
    }
    // drop the inscription entry
    diasto.inscripts.drop(idx)?;
    diasto.prev_engraved_height = BlockHeight::from(pdhei);
    state.diamond_set(&this.diamond, &diasto);
    // burn fee
    if pfee.is_positive() {
        let mut ttcount = state.get_total_count();
        let pfee_zhu = pfee.to_zhu_u64()?;
        let burn_total = (*ttcount.diamond_insc_burn_zhu)
            .checked_add(pfee_zhu)
            .ok_or_else(|| "diamond_insc_burn_zhu overflow".to_string())?;
        ttcount.diamond_insc_burn_zhu = Uint8::from(burn_total);
        state.set_total_count(&ttcount);
        hac_sub(ctx, &main_addr, &pfee)?;
    }
    Ok(vec![])
}

/************************************ */

action_define! { DiamondInscriptionEdit, 36,
    ActLv::MainCall, // level
    true, // burn 90% fee
    [], // need sign
    {
        diamond           : DiamondName
        index             : Uint1
        protocol_cost     : Amount
        engraved_type     : Uint1
        engraved_content  : BytesW1
    },
    (self, {
        let a = self;
        let ins_str = a.engraved_content.to_readable_or_hex();
        let mut desc = format!("Edit inscription #{} of HACD {} to \"{}\"",
            *a.index, a.diamond.to_readable(), ins_str);
        if a.protocol_cost.is_positive() {
            desc += &format!(" cost {} HAC fee", a.protocol_cost.to_fin_string());
        }
        desc
    }),
    (self, ctx, _gas {
        #[cfg(not(feature = "hip22"))]
        if true { return errf!("HIP-22 not activated") }
        diamond_inscription_edit(self, ctx)
    })
}

fn diamond_inscription_edit(this: &DiamondInscriptionEdit, ctx: &mut dyn Context) -> Ret<Vec<u8>> {
    let env = ctx.env().clone();
    let main_addr = env.tx.main;
    let pfee = &this.protocol_cost;
    if pfee.is_negative() {
        return errf!("protocol cost cannot be negative");
    }
    if pfee.size() > 4 {
        return errf!("protocol cost amount size cannot over 4 bytes");
    }
    ctx.check_sign(&main_addr)?;
    // check inscription content
    check_inscription_content(*this.engraved_type, &this.engraved_content)?;
    let idx = *this.index as usize;
    let pdhei = env.block.height;
    // check diamond
    let mut state = CoreState::wrap(ctx.state());
    let mut diasto = check_diamond_status_for_inscription(&mut state, &main_addr, &this.diamond)?;
    let cur_len = diasto.inscripts.length();
    if cur_len == 0 {
        return errf!("no inscriptions in HACD {}", this.diamond.to_readable());
    }
    if idx >= cur_len {
        return errf!(
            "inscription index {} out of range, HACD {} has {} inscriptions",
            idx,
            this.diamond.to_readable(),
            cur_len
        );
    }
    // check cooldown
    check_inscription_cooldown(*diasto.prev_engraved_height, pdhei, &this.diamond)?;
    // protocol cost: average_bid_burn / 100
    let diaslt = must_have!(
        format!("diamond {}", this.diamond.to_readable()),
        state.diamond_smelt(&this.diamond)
    );
    let cost = calc_edit_inscription_protocol_cost(*diaslt.average_bid_burn);
    if pfee < &cost {
        return errf!(
            "inscription edit cost error need {:?} but got {:?}",
            cost,
            pfee
        );
    }
    // replace the inscription entry
    diasto
        .inscripts
        .replace(idx, this.engraved_content.clone())?;
    diasto.prev_engraved_height = BlockHeight::from(pdhei);
    state.diamond_set(&this.diamond, &diasto);
    // burn protocol fee
    if pfee.is_positive() {
        let mut ttcount = state.get_total_count();
        let pfee_zhu = pfee.to_zhu_u64()?;
        let burn_total = (*ttcount.diamond_insc_burn_zhu)
            .checked_add(pfee_zhu)
            .ok_or_else(|| "diamond_insc_burn_zhu overflow".to_string())?;
        ttcount.diamond_insc_burn_zhu = Uint8::from(burn_total);
        state.set_total_count(&ttcount);
        hac_sub(ctx, &main_addr, pfee)?;
    }
    // ok
    Ok(vec![])
}

/************************************ */

#[inline]
pub fn calc_append_inscription_protocol_cost(
    cur_inscriptions: usize,
    average_bid_burn_mei: u16,
) -> Amount {
    if cur_inscriptions < APPEND_FREE_MAX_INSCRIPTIONS {
        return Amount::zero();
    }
    if cur_inscriptions < APPEND_TIER1_MAX_INSCRIPTIONS {
        // 11~40: average_bid_burn / 50
        return Amount::coin(average_bid_burn_mei as u64 * 2, 246);
    }
    if cur_inscriptions < APPEND_TIER2_MAX_INSCRIPTIONS {
        // 41~100: average_bid_burn / 20
        return Amount::coin(average_bid_burn_mei as u64 * 5, 246);
    }
    // 101~200: average_bid_burn / 10
    Amount::coin(average_bid_burn_mei as u64 * 10, 246)
}

#[inline]
pub fn calc_move_inscription_protocol_cost(
    target_cur_inscriptions: usize,
    average_bid_burn_mei: u16,
) -> Amount {
    calc_append_inscription_protocol_cost(target_cur_inscriptions, average_bid_burn_mei)
}

#[inline]
pub fn calc_edit_inscription_protocol_cost(average_bid_burn_mei: u16) -> Amount {
    // average_bid_burn / 100
    Amount::coin(average_bid_burn_mei as u64, 246)
}

#[inline]
pub fn calc_drop_inscription_protocol_cost(average_bid_burn_mei: u16) -> Amount {
    // average_bid_burn / 50
    Amount::coin(average_bid_burn_mei as u64 * 2, 246)
}

/**
*
* return total cost
*/
pub fn engraved_one_diamond(
    pending_height: u64,
    state: &mut CoreState,
    addr: &Address,
    diamond: &DiamondName,
    content: &BytesW1,
) -> Ret<Amount> {
    let mut diasto = check_diamond_status_for_inscription(state, addr, diamond)?;
    // check height
    check_inscription_cooldown(*diasto.prev_engraved_height, pending_height, diamond)?;
    // check insc
    let haveng = diasto.inscripts.length();
    if haveng >= INSCRIPTION_MAX_PER_DIAMOND {
        return errf!(
            "maximum inscriptions for one diamond is {}",
            INSCRIPTION_MAX_PER_DIAMOND
        );
    }
    let diaslt = must_have!(
        format!("diamond {}", diamond.to_readable()),
        state.diamond_smelt(&diamond)
    );
    // cost: stepped append protocol fee
    let cost = calc_append_inscription_protocol_cost(haveng, *diaslt.average_bid_burn);
    // do engraved
    diasto.prev_engraved_height = BlockHeight::from(pending_height);
    diasto.inscripts.push(content.clone())?;
    // save
    state.diamond_set(diamond, &diasto);
    // ok finish
    Ok(cost)
}

/*
* return total cost
*/
pub fn engraved_clean_one_diamond(
    pending_height: u64,
    state: &mut CoreState,
    addr: &Address,
    diamond: &DiamondName,
) -> Ret<Amount> {
    let mut diasto = check_diamond_status_for_inscription(state, addr, diamond)?;
    let diaslt = must_have!(
        format!("diamond {}", diamond.to_readable()),
        state.diamond_smelt(&diamond)
    );
    // check
    if diasto.inscripts.length() == 0 {
        return errf!(
            "cannot find any inscriptions in HACD {}",
            diamond.to_readable()
        );
    }
    // check height cooldown
    check_inscription_cooldown(*diasto.prev_engraved_height, pending_height, diamond)?;
    // burning cost bid fee
    let cost = Amount::mei(*diaslt.average_bid_burn as u64);
    // do clean
    diasto.prev_engraved_height = BlockHeight::from(pending_height);
    diasto.inscripts = Inscripts::default();
    // save
    state.diamond_set(diamond, &diasto);
    // ok finish
    Ok(cost)
}
