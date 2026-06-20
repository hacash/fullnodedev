// HIP-25: Diamond Stake / Unstake (kinds 37 / 38 on fullnodedev).

action_define! { DiaStake, 37,
    ActScope::TOP, 2, false, [],
    {
        diamonds : DiamondNameListMax200
    },
    (self, {
        let a = self;
        format!("Stake {} HACD ({})", a.diamonds.length(), a.diamonds.splitstr())
    }),
    (self, ctx, _gas {
        diamond_stake(self, ctx)
    })
}

action_define! { DiaUnstake, 38,
    ActScope::TOP, 2, false, [],
    {
        diamonds : DiamondNameListMax200
    },
    (self, {
        let a = self;
        format!("Unstake {} HACD ({})", a.diamonds.length(), a.diamonds.splitstr())
    }),
    (self, ctx, _gas {
        diamond_unstake(self, ctx)
    })
}

fn diamond_stake(this: &DiaStake, ctx: &mut dyn Context) -> XRet<Vec<u8>> {
    let env = ctx.env().clone();
    let staker = env.tx.main;
    ctx.check_sign(&staker)?;
    let height = env.block.height;
    let activation = env.chain.staking_activation_height;
    let mut state = CoreState::wrap(ctx.state());
    staking_apply_stake(&mut state, &staker, &this.diamonds, height, activation)?;
    Ok(vec![])
}

fn diamond_unstake(this: &DiaUnstake, ctx: &mut dyn Context) -> XRet<Vec<u8>> {
    let env = ctx.env().clone();
    let staker = env.tx.main;
    ctx.check_sign(&staker)?;
    let height = env.block.height;
    let mut state = CoreState::wrap(ctx.state());
    staking_apply_unstake(&mut state, &staker, &this.diamonds, height)?;
    Ok(vec![])
}