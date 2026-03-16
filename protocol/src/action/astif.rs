action_define!{ AstIf, 26,
    ActScope::AST,
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
        gas = 0; // control-flow node: all gas consumed via ctx
        let mut guard = ast_enter(ctx)?;
        let ctx = guard.ctx();
        // Whole-node savepoint: if branch execution fails, rollback both
        // condition side effects and branch side effects.
        let node = AstNodeTxn::begin(ctx)?;
        let res = self.execute_if_core(ctx);
        node.finish(ctx, res)
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

    fn execute_if_core(&self, ctx: &mut dyn Context) -> Ret<Vec<u8>> {
        let cond_ok = ast_revert_continue(ast_try_item!(ctx, self.cond.execute(ctx), self.cond.burn_90()))?.is_some();
        let branch = maybe!(cond_ok, &self.br_if, &self.br_else);
        let ret = ast_try_item!(ctx, branch.execute(ctx), branch.burn_90()).into_tret()?;
        Ok(ret)
    }
}
