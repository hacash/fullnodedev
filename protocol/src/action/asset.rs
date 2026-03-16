



action_define!{ AssetToTrs, 17, 
    ActScope::CALL, true, [], 
    {
        to: AddrOrPtr
        asset: AssetAmt
    },
    (self, format!("Transfer {} to {}", self.asset, self.to.to_readable())),
    (self, ctx, _gas {
        let from = ctx.env().tx.main; 
        let to   = ctx.addr(&self.to)?;
        asset_transfer(ctx, &from, &to, &self.asset)
    })
}


action_define!{ AssetFromTrs, 18, 
    ActScope::CALL, true, 
    [ 
        self.from
    ], 
    {
        from: AddrOrPtr
        asset: AssetAmt
    },
    (self, format!("Transfer {} from {}", self.asset, self.from.to_readable())),
    (self, ctx, _gas {
        let from = ctx.addr(&self.from)?;
        let to   = ctx.env().tx.main; 
        asset_transfer(ctx, &from, &to, &self.asset)
    })
}


action_define!{ AssetFromToTrs, 19, 
    ActScope::CALL, true,
    [ 
        self.from
    ], 
    {
        from: AddrOrPtr
        to: AddrOrPtr
        asset: AssetAmt
    },
    (self, format!("Transfer {} from {} to {}", self.asset, self.from.to_readable(), self.to.to_readable())),
    (self, ctx, _gas {
        let from = ctx.addr(&self.from)?;
        let to   = ctx.addr(&self.to)?;
        asset_transfer(ctx, &from, &to, &self.asset)
    })
}




