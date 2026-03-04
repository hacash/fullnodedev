

combi_list!{ ChainIDList, Uint1, Uint4}
macro_rules! cids_to_str { ($cids:expr) => {
    $cids.iter().map(|c: &Uint4|c.to_string()).collect::<Vec<_>>().join(",")
}}

action_define!{ ChainAllow, 0x0411, 
    ActLv::Guard, false, [],
    {
        chains: ChainIDList
    },
    (self, format!("Valid chain ID list {}", cids_to_str!(self.chains.as_list()))),
    (self, ctx, _gas {
        let cid = ctx.env().chain.id;
        let ids = self.chains.as_list();
        if ! ids.iter().any(|id| id.uint() == cid) {
            let cids = cids_to_str!(ids);
            return erruf!("transction must belong to chains {} but on chain {}", cids, cid)
        }
        // ok
        Ok(vec![])
    })
}


/*
*
*/
action_define!{ HeightScope, 0x0412, 
    ActLv::Guard, false, [],
    {
        start: BlockHeight
        end:   BlockHeight
    },
    (self, format!("Limit height range ({}, {})", 
        *self.start, if *self.end == 0 { "Unlimited".to_owned() } else { self.end.to_string() })),
    (self, ctx, _gas {
        let pdhei = ctx.env().block.height;
        let left = *self.start;
        let right = match *self.end {
            0 => u64::MAX,
            h => h,
        };
        if left > right {
            return errf!("left height {} cannot big than rigth height {}", left, right)
        }
        if pdhei < left || pdhei > right {
            return erruf!("transction must submit in height between {} and {}", left, right)
        }
        // ok
        Ok(vec![])
    })
}




action_define!{ BalanceFloor, 0x0413,
    ActLv::Guard, false, [],
    {
        addr    : AddrOrPtr
        hacash  : Amount
        satoshi : Satoshi
        diamond : DiamondNumber
        assets  : AssetAmtW1
    },
    (self, format!(
        "Balance floor for {} (hac={}, sat={}, dia={}, assets={})",
        self.addr.to_readable(),
        self.hacash,
        *self.satoshi,
        *self.diamond,
        self.assets.length()
    )),
    (self, ctx, _gas {
        if self.hacash.is_negative() {
            return errf!("balance floor hacash {} cannot be negative", self.hacash)
        }
        check_balance_floor_assets(&self.assets)?;
        let check_hac = !self.hacash.is_zero();
        let check_sat = self.satoshi.uint() > 0;
        let check_dia = self.diamond.uint() > 0;
        let check_assets = self.assets.length() > 0;
        if !(check_hac || check_sat || check_dia || check_assets) {
            return errf!("balance floor is empty")
        }
        let adr = ctx.addr(&self.addr)?;
        let bls = CoreState::wrap(ctx.state()).balance(&adr).unwrap_or_default();
        if check_hac && bls.hacash < self.hacash {
            return erruf!(
                "address {} hacash {} is lower than floor {}",
                adr, bls.hacash, self.hacash
            )
        }
        if check_sat {
            let sat = bls.satoshi.to_satoshi();
            if sat < self.satoshi {
                return erruf!(
                    "address {} satoshi {} is lower than floor {}",
                    adr, sat, self.satoshi
                )
            }
        }
        if check_dia {
            let dia = bls.diamond.to_diamond();
            if dia < self.diamond {
                return erruf!(
                    "address {} diamond {} is lower than floor {}",
                    adr, dia, self.diamond
                )
            }
        }
        for floor in self.assets.as_list() {
            let cur = bls
                .asset(floor.serial)
                .unwrap_or(AssetAmt::from_serial(floor.serial));
            if cur.amount < floor.amount {
                return erruf!(
                    "address {} asset {}:{} is lower than floor {}:{}",
                    adr,
                    cur.serial,
                    cur.amount,
                    floor.serial,
                    floor.amount
                )
            }
        }
        Ok(vec![])
    })
}

fn check_balance_floor_assets(assets: &AssetAmtW1) -> Rerr {
    if assets.length() > BALANCE_ASSET_MAX {
        return errf!(
            "balance floor assets item quantity cannot big than {}",
            BALANCE_ASSET_MAX
        )
    }
    let mut seen = std::collections::HashSet::new();
    for ast in assets.as_list() {
        let serial = ast.serial.uint();
        let amount = ast.amount.uint();
        if serial == 0 {
            return errf!("balance floor asset serial cannot be zero")
        }
        if amount == 0 {
            return errf!(
                "balance floor asset {} amount cannot be zero",
                serial
            )
        }
        if !seen.insert(serial) {
            return errf!("balance floor asset serial {} is duplicate", serial)
        }
    }
    Ok(())
}
