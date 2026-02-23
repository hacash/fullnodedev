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
        ast_with_whole_snapshot(ctx, |ctx| {
            let cond_ok = match ast_run_item_snapshot(ctx, &mut gas, |ctx| self.cond.execute(ctx))? {
                Ok((_cond_gas, _)) => true,
                Err(e) => {
                    if e.is_interrupt() {
                        return ast_rethrow(e)
                    }
                    false
                }
            };
            let branch = if cond_ok { &self.br_if } else { &self.br_else };
            match ast_run_shared_gas(ctx, &mut gas, |ctx| branch.execute(ctx))? {
                Ok((_branch_gas, ret)) => Ok(ret),
                Err(e) => ast_rethrow(e),
            }
        })
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
