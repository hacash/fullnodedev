macro_rules! purity_or_fee {
    ($self:ident, $txp:expr, $opt:tt, $hav:expr) => {
        maybe!($self.fpmd, $txp.fpur() $opt $hav.fpur(), $txp.tx().fee() $opt $hav.tx().fee() )
    };
}

fn scan_group_rng_by_feep(
    txpkgs: &Vec<TxPkg>,
    feep: u64,
    fee: &Amount,
    fpmd: bool,
    wsz: (usize, usize),
) -> (usize, usize) {
    let mut rxl = wsz.0;
    let mut rxr = wsz.1;
    loop {
        let rng = rxr - rxl;
        if rng <= 10 {
            break;
        }
        let fct = rxl + rng / 2;
        let ct = &txpkgs[fct];
        let lcd = maybe!(fpmd, feep > ct.fpur(), fee > ct.tx().fee());
        let rcd = maybe!(fpmd, feep < ct.fpur(), fee < ct.tx().fee());
        if lcd {
            rxr = fct;
        } else if rcd {
            rxl = fct;
        } else {
            break;
        }
    }
    (rxl, rxr)
}
