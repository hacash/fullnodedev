action_define! { AstIf, 26,
    ActLv::Ast, // level
    // burn 90 fee , check child burn 90
    self.cond.burn_90() || self.br_if.burn_90() || self.br_else.burn_90(),
    self.collect_req_sign(),
    {
        cond:    AstSelect
        br_if:   AstSelect
        br_else: AstSelect
    },
    (self, "Asset if-else execute".to_owned()),
    (self, ctx, gas {
        #[cfg(not(feature = "ast"))]
        if true {
            return errf!("ast if not open")
        }
        let mut guard = ast_enter(ctx)?;
        let ctx = guard.ctx();
        // Whole-node savepoint: if branch execution fails, rollback both
        // condition side effects and branch side effects.
        let whole_snap = ctx_snapshot(ctx)?;
        let snap_before = ctx.gas_remaining();
        let snap = ast_item_snapshot(ctx)?;
        gas_add(&mut gas, ast_gas_spent_delta(ctx, snap_before));
        let cond_before = ctx.gas_remaining();
        let cond_res = self.cond.execute(ctx);
        let cond_shared = ast_gas_spent_delta(ctx, cond_before);
        gas_add(&mut gas, cond_shared);
        let branch_before = ctx.gas_remaining();
        let branch_res = match cond_res {
            // if br
            Ok((cond_gas, ..)) => {
                if cond_gas < 0 {
                    return errf!("negative returned gas: {}", cond_gas)
                }
                let cond_extra = cond_gas.saturating_sub(cond_shared).max(0);
                gas_add(&mut gas, cond_extra);
                ctx_merge(ctx, snap);
                self.br_if.execute(ctx)
            },
            // else br
            Err(e) => {
                if e.is_unrecoverable() {
                    ctx_recover(ctx, whole_snap)?;
                    return ast_rethrow(e)
                }
                ctx_recover(ctx, snap)?;
                self.br_else.execute(ctx)
            }
        };
        let branch_shared = ast_gas_spent_delta(ctx, branch_before);
        gas_add(&mut gas, branch_shared);
        let branch_ret = match branch_res {
            Ok((branch_gas, ret)) => {
                if branch_gas < 0 {
                    return errf!("negative returned gas: {}", branch_gas)
                }
                let branch_extra = branch_gas.saturating_sub(branch_shared).max(0);
                gas_add(&mut gas, branch_extra);
                ret
            },
            Err(e) => {
                ctx_recover(ctx, whole_snap)?;
                return ast_rethrow(e)
            }
        };
        ctx_merge(ctx, whole_snap);
        Ok(branch_ret)
    })
}

impl AstIf {
    pub fn create_by(cond: AstSelect, br_if: AstSelect, br_else: AstSelect) -> Self {
        Self {
            cond,
            br_if,
            br_else,
            ..Self::new()
        }
    }

    pub(crate) fn collect_req_sign(&self) -> Vec<AddrOrPtr> {
        let mut req = self.cond.collect_req_sign();
        req.extend(self.br_if.collect_req_sign());
        req.extend(self.br_else.collect_req_sign());
        req
    }
}
