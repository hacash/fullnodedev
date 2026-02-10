


action_define!{AstIf, 22, 
    ActLv::Ast, // level
    // burn 90 fee , check child burn 90
    self.cond.burn_90() || self.br_if.burn_90() || self.br_else.burn_90(), 
    [],
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
        //
        let snap = ctx_snapshot(ctx);
        match self.cond.execute(ctx) {
            // if br
            Ok(..) => {
                ctx_merge(ctx, snap);
                self.br_if.execute(ctx)
            },
            // else br
            Err(..) => {
                ctx_recover(ctx, snap);
                self.br_else.execute(ctx)
            }
        }.map(|(_,b)|b)
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

}


