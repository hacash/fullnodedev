


action_define!{AstIf, 22, 
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
        let snap = ctx_snapshot(ctx);
        let cond_res = self.cond.execute(ctx);
        let (cond_gas, branch_res) = match cond_res {
            // if br
            Ok((g, ..)) => {
                ctx_merge(ctx, snap);
                (g, self.br_if.execute(ctx))
            },
            // else br
            Err(..) => {
                ctx_recover(ctx, snap);
                (0, self.br_else.execute(ctx))
            }
        };
        let (branch_gas, branch_ret) = branch_res?;
        gas += cond_gas;
        gas += branch_gas;
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
