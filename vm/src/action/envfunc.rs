/**************** env *****************/

action_define! { EnvHeight, 0x0701,
    ActScope::CALL_ONLY, 3, false, [], {},
    (self, "Syscall: Get block height".to_owned()),
    (self, ctx, _gas {
        Ok(ctx.env().block.height.to_be_bytes().to_vec())
    })
}

action_define! { EnvMainAddr, 0x0702,
    ActScope::CALL_ONLY, 3, false, [], {},
    (self, "Syscall: Get main address".to_owned()),
    (self, ctx, _gas {
        Ok(ctx.env().tx.main.to_vec())
    })
}

action_define! { EnvCoinbaseAddr, 0x0703,
    ActScope::CALL_ONLY, 3, false, [], {},
    (self, "Syscall: Get coinbase address".to_owned()),
    (self, ctx, _gas {
        let cbadr = ctx.env().block.coinbase.clone();
        Ok(cbadr.to_vec())
    })
}



/**************** view *****************/



action_define! { ViewBalance, 0x0601,
    ActScope::CALL_ONLY, 3, false, [],
    {
        addr: Address
    },
    (self, format!("Syscall: Get balance for {}", self.addr)),
    (self, ctx, _gas {
        let bls = CoreState::wrap(ctx.state()).balance(&self.addr).unwrap_or_default();
        let dia = bls.diamond.uint();
        if dia > u32::MAX as u64 {
            return xerrf!("address {} diamond count {} exceeds u32::MAX", self.addr, dia);
        }
        let hac = bls.hacash.serialize();
        let mut res = Vec::with_capacity(12 + hac.len());
        res.extend_from_slice(&(dia as u32).to_be_bytes());
        res.extend_from_slice(&bls.satoshi.uint().to_be_bytes());
        res.extend_from_slice(&hac);
        Ok(res)
    })
}


action_define! { ViewAssetBalance, 0x0602,
    ActScope::CALL_ONLY, 3, false, [],
    {
        addr: Address
        serial: Fold64
    },
    (self, format!("Syscall: Get asset {} balance for {}", self.serial, self.addr)),
    (self, ctx, _gas {
        let serial = self.serial.uint();
        if serial == 0 {
            return xerrf!("asset serial cannot be zero")
        }
        let bls = CoreState::wrap(ctx.state()).balance(&self.addr).unwrap_or_default();
        let amt = bls
            .assets
            .as_list()
            .iter()
            .find(|a| a.serial.uint() == serial)
            .map(|a| a.amount.uint())
            .unwrap_or(0);
        Ok(amt.to_be_bytes().to_vec())
    })
}



action_define! { ViewCheckSign, 0x0609,
    ActScope::CALL_ONLY, 3, false, [],
    {
        addr: Address
    },
    (self, format!("Syscall: Check signature for {}", self.addr)),
    (self, ctx, _gas {
        match ctx.check_sign(&self.addr) {
            Ok(..) => Ok(vec![1]), // yes
            _ => Ok(vec![0]) // no
        }
    })
}

action_define! { ViewDiaInscNum, 0x0611,
    ActScope::CALL_ONLY, 3, false, [],
    {
        diamond: DiamondName
    },
    (self, format!("Syscall: Get diamond inscription number for <{}>", self.diamond.to_readable())),
    (self, ctx, _gas {
        let Some(diaobj) = CoreStateRead::wrap(ctx.state()).diamond(&self.diamond) else {
            return xerrf!("diamond {} not found", self.diamond.to_readable())
        };
        let num = diaobj.inscripts.length();
        if num > u8::MAX as usize {
            return xerrf!("diamond {} inscripts number invalid", self.diamond.to_readable())
        }
        // ok
        Ok(vec![num as u8])
    })
}

action_define! { ViewDiaInscGet, 0x0612,
    ActScope::CALL_ONLY, 3, false, [],
    {
        diamond: DiamondName
        inscidx: Uint1
    },
    (self, format!("Syscall: Get diamond inscription data for <{}>", self.diamond.to_readable())),
    (self, ctx, _gas {
        let Some(diaobj) = CoreStateRead::wrap(ctx.state()).diamond(&self.diamond) else {
            return xerrf!("diamond {} not found", self.diamond.to_readable())
        };
        let num = diaobj.inscripts.length();
        let idx = self.inscidx.uint() as usize ;
        if idx >= num {
            return xerrf!("diamond {} inscripts number overflow", self.diamond.to_readable())
        }
        let insc = &diaobj.inscripts.as_list()[idx];
        // ok
        Ok(insc.content.to_vec())
    })
}

action_define! { ViewDiaNameList, 0x0613,
    ActScope::CALL_ONLY, 3, false, [],
    {
        addr: Address
        page: DiamondNumber
        limit: DiamondNumber
    },
    (self, format!("Syscall: Get HACD name list for {} page {} limit {}", self.addr, self.page, self.limit)),
    (self, ctx, _gas {
        const DNM_SZ: usize = DiamondName::SIZE;
        let owned = CoreStateRead::wrap(ctx.state()).diamond_owned(&self.addr).unwrap_or_default();
        let names = owned.names.as_ref();
        if names.len() % DNM_SZ != 0 {
            return xerrf!("address {} diamond names length {} invalid", self.addr, names.len())
        }
        let limit = self.limit.uint() as usize;
        if limit > 200 {
            return xerrf!("limit {} cannot exceed 200", limit)
        }
        if limit == 0 {
            return Ok(vec![])
        }
        let page = self.page.uint() as usize;
        let unit = limit * DNM_SZ;
        let start = page.saturating_mul(unit);
        if start >= names.len() {
            return Ok(vec![])
        }
        let end = start.saturating_add(unit).min(names.len());
        Ok(names[start..end].to_vec())
    })
}

action_define! { ViewDiaOwnerAddrs, 0x0614,
    ActScope::CALL_ONLY, 3, false, [],
    {
        diamonds: DiamondNameListMax200
    },
    (self, format!("Syscall: Get HACD owner addresses for {}", self.diamonds.splitstr())),
    (self, ctx, _gas {
        let num = self.diamonds.check()?;
        if num > 50 {
            return xerrf!("diamond list length {} cannot exceed 50", num)
        }
        let state = CoreStateRead::wrap(ctx.state());
        let mut res = Vec::with_capacity(num * Address::SIZE);
        for dian in self.diamonds.as_list() {
            let Some(diaobj) = state.diamond(dian) else {
                return xerrf!("diamond {} not found", dian.to_readable())
            };
            res.extend_from_slice(diaobj.address.as_ref());
        }
        Ok(res)
    })
}
