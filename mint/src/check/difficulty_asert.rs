const ASERT_UPGRADE_HEIGHT: u64 = 738654;
const ASERT_START_TARGET_NUM: u32 = 0xe9cf_ffff;

const ASERT_HALF_LIFE: i64 = 10800;
const ASERT_RADIX_BITS: u32 = 16;
const ASERT_RADIX: i64 = 1i64 << ASERT_RADIX_BITS;
const ASERT_POLY_1: u128 = 195766423245049;
const ASERT_POLY_2: u128 = 971821376;
const ASERT_POLY_3: u128 = 5127;
const ASERT_POLY_TERM_SHIFT: usize = 48;
const ASERT_EASING_MAX_SCALE: u8 = 2;

impl DifficultyGnr {
    fn asert_anchor_height(&self) -> u64 {
        self.asert_upgrade_height()
    }

    fn target_asert(
        &self,
        prevdiff: u32,
        _prevblkt: u64,
        hei: u64,
        blkt: u64,
        src: &dyn BlockIntroSource,
    ) -> DifficultyTarget {
        let upgrade_hei = self.asert_upgrade_height();
        if hei == upgrade_hei {
            return DifficultyTarget::from_num(ASERT_START_TARGET_NUM)
        }
        let anchor_hei = self.asert_anchor_height();
        let anchor_time = self.req_block_intro(anchor_hei, src).0;
        let anchor_target = u32_to_biguint(ASERT_START_TARGET_NUM);
        let eval_time = blkt as i128;
        let eval_hei = hei as i128;
        let time_delta = eval_time - anchor_time as i128;
        let height_delta = eval_hei - anchor_hei as i128;
        let exponent = ((time_delta - self.cnf.each_block_target_time as i128 * height_delta) * ASERT_RADIX as i128) / ASERT_HALF_LIFE as i128;
        let num_shifts = exponent >> ASERT_RADIX_BITS;
        let frac = (exponent - (num_shifts << ASERT_RADIX_BITS)) as u128;
        let frac2 = frac * frac;
        let frac3 = frac2 * frac;
        let factor = (((ASERT_POLY_1 * frac + ASERT_POLY_2 * frac2 + ASERT_POLY_3 * frac3 + (1u128 << (ASERT_POLY_TERM_SHIFT - 1))) >> ASERT_POLY_TERM_SHIFT) + ASERT_RADIX as u128) as u64;
        let prev_target = u32_to_biguint(prevdiff);
        let ease_target = prev_target * BigUint::from(ASERT_EASING_MAX_SCALE);
        let max_target = u32_to_biguint(LOWEST_DIFFICULTY);
        let mut next_target = anchor_target * BigUint::from(factor);
        if num_shifts < 0 {
            next_target >>= (-num_shifts) as usize;
        } else if num_shifts > 0 {
            next_target <<= num_shifts as usize;
        }
        next_target >>= ASERT_RADIX_BITS as usize;
        if next_target == BigUint::default() {
            return DifficultyTarget::from_big(BigUint::from(1u8))
        }
        if next_target > ease_target {
            next_target = ease_target;
        }
        if next_target > max_target {
            return DifficultyTarget::from_num(LOWEST_DIFFICULTY)
        }
        DifficultyTarget::from_big(next_target)
    }
}
