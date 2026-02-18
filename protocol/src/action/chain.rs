
/*
*
*/
action_define!{ HeightScope, 0x0411, 
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
            return errf!("transction must submit in height between {} and {}", left, right)
        }
        // ok
        Ok(vec![])
    })
}


combi_list!{ ChainIDList, Uint1, Uint4}
macro_rules! cids_to_str { ($cids:expr) => {
    $cids.iter().map(|c: &Uint4|c.to_string()).collect::<Vec<_>>().join(",")
}}

action_define!{ ChainAllow, 0x0412, 
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
            return errf!("transction must belong to chains {} but on chain {}", cids, cid)
        }
        // ok
        Ok(vec![])
    })
}
