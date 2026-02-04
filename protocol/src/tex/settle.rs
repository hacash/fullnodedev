// use crate::operate::hacd_transfer;



pub fn do_settlement(ctx: &mut dyn Context) -> Rerr {
    // Validate and materialize settlement operations while holding a short borrow to the ledger.
    // Then execute state mutations without holding `&mut TexLedger` to avoid borrow cycles.
    let mut diamond_trs: Vec<(Address, DiamondNameListMax200)> = vec![];
    {
        let t = ctx.tex_ledger();
        if t.zhu != 0 || t.sat != 0 || t.dia != 0 {
            return errf!("coin settlement check failed")
        }
        for (a, v) in t.assets.iter() {
            if *v != 0 {
                return errf!("asset <{}> settlement check failed", a.uint())
            }
        }
        // settle diamonds (fetch_list() drains from ledger)
        for (adr, dn) in &t.diatrs {
            let dialist = DiamondNameListMax200::from_list_checked(t.diamonds.fetch_list(*dn)?)?;
            diamond_trs.push((adr.clone(), dialist));
        }
        // after fetch_list(), ledger should have no diamonds remaining
        if t.diamonds.length() > 0 {
            return errf!("diamonds settlement check failed")
        }
    }
    for (adr, dialist) in diamond_trs {
        let _ = do_diamonds_transfer(&dialist, &SETTLEMENT_ADDR, &adr, ctx)?;
    }
    Ok(())
}