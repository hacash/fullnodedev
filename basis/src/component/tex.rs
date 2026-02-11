

/*********************************/


#[allow(dead_code)]
#[derive(Default, Clone)]
pub struct TexLedger {
    pub zhu: i64,
    pub sat: i64,
    pub dia: i32,
    pub diamonds: DiamondNameListMax60000,
    pub diatrs:   Vec<(Address, usize)>,
    pub assets:   HashMap<Fold64, i128>,
    pub asset_checked: HashSet<Fold64>,
}


impl TexLedger {

    pub fn record_diamond_pay(&mut self, dias: DiamondNameListMax200) -> Rerr {
        let Some(newdia) = self.dia.checked_add(dias.length() as i32) else {
            return errf!("cell state diamond record overflow")
        };
        self.dia = newdia;
        self.diamonds.checked_append(dias.into_list())
    }
    
    pub fn record_diamond_get(&mut self, addr: &Address, num: usize) -> Rerr {
        if num > 200 {
            return errf!("Tex state diamond trs num cannot over 200")
        }
        self.diatrs.push((addr.clone(), num));
        let Some(diares) = self.dia.checked_sub(num as i32) else {
            return errf!("cell state diamond overflow")
        };
        self.dia = diares;
        Ok(())
    }

}
