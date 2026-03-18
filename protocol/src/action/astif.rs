action_define! { AstIf, 26,
    ActScope::AST, 3, false,
    self.collect_req_sign(),
    {
        cond:    AstSelect
        br_if:   AstSelect
        br_else: AstSelect
    },
    (self, "Asset if-else execute".to_owned()),
    (self, ctx, gas {
        gas = 0; // control-flow node: all gas consumed via ctx
        let mut exec_from = enter_exec_from(ctx, ExecFrom::Ast);
        let ctx = exec_from.ctx();
        // Failed branches bubble up directly; only recoverable child items roll back their own snapshots.
        self.execute_if_core(ctx)
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

    fn execute_if_core(&self, ctx: &mut dyn Context) -> XRet<Vec<u8>> {
        let cond_ok = match ast_exec_item(ctx, self.cond.extra9(), |ctx| self.cond.execute(ctx)) {
            Ok(_) => true,
            Err(XError::Revert(_)) => false,
            Err(e) => return Err(e),
        };
        let branch = maybe!(cond_ok, &self.br_if, &self.br_else);
        let ret = ast_exec_item(ctx, branch.extra9(), |ctx| branch.execute(ctx))?;
        Ok(ret)
    }
}
