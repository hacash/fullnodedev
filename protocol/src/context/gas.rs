
const TX_GAS_BUDGET_CAP: i64 = 8192;

/// - `table[i] = floor(138 * 1.07^i)`, `i in 0..=255`.
/// - `table[255] = 4,292,817,207` (fits in `u32`).
/// start from 138 but set it 0
/// Runtime decoding keeps `gas_max=0` as a reserved "no VM gas" value.
/// Therefore `decode_gas_budget(0)=0`, while non-zero bytes use the lookup table.
pub const GAS_BUDGET_LOOKUP_1P07_FROM_138: [u32; 256] = [
    0,   147, 157, 169, 180, 193, 207, 221, 237, 253,
    271, 290, 310, 332, 355, 380, 407, 435, 466, 499,
    534, 571, 611, 654, 699, 748, 801, 857, 917, 981,
    1050, 1124, 1202, 1286, 1376, 1473, 1576, 1686, 1804, 1931,
    2066, 2211, 2365, 2531, 2708, 2898, 3101, 3318, 3550, 3799,
    4065, 4349, 4654, 4979, 5328, 5701, 6100, 6527, 6984, 7473,
    7996, 8556, 9155, 9796, 10481, 11215, 12000, 12840, 13739, 14701,
    15730, 16831, 18009, 19270, 20619, 22062, 23607, 25259, 27027, 28919,
    30944, 33110, 35428, 37908, 40561, 43401, 46439, 49689, 53168, 56889,
    60872, 65133, 69692, 74571, 79791, 85376, 91352, 97747, 104589, 111911,
    119744, 128126, 137095, 146692, 156961, 167948, 179704, 192284, 205743, 220146,
    235556, 252045, 269688, 288566, 308766, 330379, 353506, 378251, 404729, 433060,
    463374, 495811, 530517, 567654, 607389, 649907, 695400, 744078, 796164, 851895,
    911528, 975335, 1043608, 1116661, 1194827, 1278465, 1367958, 1463715, 1566175, 1675807,
    1793114, 1918632, 2052936, 2196642, 2350407, 2514935, 2690980, 2879349, 3080904, 3296567,
    3527327, 3774240, 4038436, 4321127, 4623606, 4947258, 5293566, 5664116, 6060604, 6484847,
    6938786, 7424501, 7944216, 8500311, 9095333, 9732006, 10413247, 11142174, 11922126, 12756675,
    13649643, 14605118, 15627476, 16721399, 17891897, 19144330, 20484433, 21918343, 23452627, 25094311,
    26850913, 28730477, 30741611, 32893523, 35196070, 37659795, 40295981, 43116699, 46134868, 49364309,
    52819811, 56517198, 60473402, 64706540, 69235998, 74082517, 79268294, 84817074, 90754270, 97107068,
    103904563, 111177883, 118960335, 127287558, 136197687, 145731525, 155932732, 166848023, 178527385, 191024302,
    204396003, 218703723, 234012984, 250393893, 267921466, 286675968, 306743286, 328215316, 351190388, 375773715,
    402077876, 430223327, 460338960, 492562687, 527042075, 563935020, 603410472, 645649205, 690844649, 739203775,
    790948039, 846314402, 905556410, 968945359, 1036771534, 1109345541, 1186999729, 1270089710, 1358995990, 1454125710,
    1555914509, 1664828525, 1781366522, 1906062178, 2039486531, 2182250588, 2335008129, 2498458698, 2673350807, 2860485364,
    3060719339, 3274969693, 3504217571, 3749512802, 4011978698, 4292817207,
];

#[inline(always)]
pub fn decode_gas_budget(b: u8) -> i64 {
    GAS_BUDGET_LOOKUP_1P07_FROM_138[b as usize] as i64
}

pub fn tx_gas_params_from_byte(gas_max_byte: u8) -> Ret<(i64, i64)> {
    if gas_max_byte == 0 {
        return errf!("gas_max_byte is 0: contract call requires tx.gas_max > 0")
    }
    let decoded = decode_gas_budget(gas_max_byte);
    let budget = decoded.min(TX_GAS_BUDGET_CAP);
    if budget <= 0 {
        return errf!(
            "gas budget invalid after clamp: gas_max_byte={} decoded={} chain_cap={}",
            gas_max_byte,
            decoded,
            TX_GAS_BUDGET_CAP
        )
    }
    Ok((budget, 1))
}

#[derive(Clone)]
struct GasCounter {
    initialized: bool,
    remaining: i64,
    initial_budget: i64,
    purity_fee: i128,
    purity_size: i128,
    gas_rate: i64,
}


impl Default for GasCounter {
    fn default() -> Self {
        Self {
            initialized: false,
            remaining: 0,
            initial_budget: 0,
            purity_fee: 0,
            purity_size: 0,
            gas_rate: 1,
        }
    }
}

impl GasCounter {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn calc_burn_amount(cost: i64, purity_fee: i128, purity_size: i128, gas_rate: i64) -> Ret<Amount> {
        if cost <= 0 {
            return errf!("gas cost invalid: {}", cost)
        }
        let gas_rate = gas_rate.max(1) as i128;
        let num = (cost as i128)
            .checked_mul(purity_fee)
            .ok_or_else(|| format!("gas burn overflow: cost={} purity_fee={}", cost, purity_fee))?;
        let den = purity_size
            .checked_mul(gas_rate)
            .ok_or_else(|| format!("gas rate overflow: purity_size={} rate={}", purity_size, gas_rate))?;
        if den <= 0 {
            return errf!("gas settle denominator invalid: purity_size={} rate={}", purity_size, gas_rate)
        }
        let burn = (num + den - 1) / den;
        if burn <= 0 {
            return errf!("gas burn underflow: cost={} purity_fee={} purity_size={} rate={}",
                cost, purity_fee, purity_size, gas_rate)
        }
        if burn > u64::MAX as i128 {
            return errf!("gas burn overflow: {}", burn)
        }
        Ok(Amount::unit238(burn as u64))
    }

    fn burn_amount(&self, cost: i64) -> Ret<Amount> {
        Self::calc_burn_amount(cost, self.purity_fee, self.purity_size, self.gas_rate)
    }
}

impl ContextInst<'_> {
    fn ctx_gas_reset(&mut self) {
        self.gas.reset();
    }

    fn ctx_gas_init_tx(&mut self, budget: i64, gas_rate: i64) -> Rerr {
        if self.gas.initialized {
            return Ok(())
        }
        if budget <= 0 {
            return errf!("gas budget invalid: {}", budget)
        }
        let tx = self.tx();
        let purity_fee = tx.fee_got().to_238_u64().unwrap_or(0) as i128;
        let purity_size = tx.size() as i128;
        if purity_fee <= 0 || purity_size <= 0 {
            return errf!("tx fee or size invalid for gas: purity_fee={} purity_size={}", purity_fee, purity_size)
        }
        
        let max_burn_amt = GasCounter::calc_burn_amount(budget, purity_fee, purity_size, gas_rate)?;
        let main = self.env().tx.main;
        crate::operate::hac_check(self, &main, &max_burn_amt)?;
        crate::operate::hac_sub(self, &main, &max_burn_amt)?;

        self.gas.remaining = budget;
        self.gas.initial_budget = budget;
        self.gas.purity_fee = purity_fee;
        self.gas.purity_size = purity_size;
        self.gas.gas_rate = gas_rate.max(1);
        self.gas.initialized = true;
        Ok(())
    }

    pub fn ctx_gas_max_charge(&self) -> Ret<Amount> {
        if !self.gas.initialized {
            return errf!("gas not initialized")
        }
        self.gas.burn_amount(self.gas.initial_budget)
    }

    pub fn ctx_gas_used_charge(&self) -> Ret<Amount> {
        if !self.gas.initialized {
            return Ok(Amount::zero())
        }
        let used = self.gas.initial_budget.saturating_sub(self.gas.remaining);
        if used <= 0 {
            return Ok(Amount::zero())
        }
        self.gas.burn_amount(used)
    }

    fn ctx_gas_refund(&mut self) -> Rerr {
        if !self.gas.initialized {
            return Ok(())
        }
        let max_charge = self.ctx_gas_max_charge()?;
        let used_charge = self.ctx_gas_used_charge()?;
        let refund = max_charge.sub_mode_u128(&used_charge)?;
        if refund.is_positive() {
            let main = self.env().tx.main;
            crate::operate::hac_add(self, &main, &refund)?;
        }
        Ok(())
    }

    fn ctx_gas_remaining(&self) -> i64 {
        self.gas.remaining
    }

    fn ctx_gas_remaining_mut(&mut self) -> Ret<&mut i64> {
        if !self.gas.initialized {
            return errf!("gas has run out")
        }
        Ok(&mut self.gas.remaining)
    }

    fn ctx_gas_consume(&mut self, gas: u32) -> Rerr {
        if !self.gas.initialized {
            return errf!("gas has run out")
        }
        self.gas.remaining -= gas as i64;
        if self.gas.remaining < 0 {
            return errf!("gas has run out")
        }
        Ok(())
    }
}
