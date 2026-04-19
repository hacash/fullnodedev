
const VK: f64 = 1000.0;
const VM: f64 = VK * VK;
const VG: f64 = VM * VK;
const VT: f64 = VG * VK;
const VP: f64 = VT * VK;
const VE: f64 = VP * VK;
const VZ: f64 = VE * VK;
const VY: f64 = VZ * VK;
const VB: f64 = VY * VK;

const HNS: [&str; 9] = ["K", "M", "G", "T", "P", "E", "Z", "Y", "B"];
const HVS: [f64; 9] = [VK, VM, VG, VT, VP, VE, VZ, VY, VB];


pub const LOWEST_DIFFICULTY: u32 = 4294967294;


/*
*
*/

pub fn rates_to_show(rates: f64) -> String {
    if !rates.is_finite() || rates <= 0.0 {
        return "0.00H/s".to_owned()
    }
    if rates < VK {
        return format!("{:.2}H/s", rates)
    }
    let mut hsx = HVS.len() - 1;
    for i in 0..HVS.len() {
        if rates < HVS[i] * VK {
            hsx = i;
            break
        }
    }
    let num = rates / HVS[hsx];
    if !num.is_finite() {
        return format!("{:.2e}H/s", rates)
    }
    format!("{:.2}{}H/s", num, HNS[hsx])
}

pub fn hash_to_rateshow(hx: &[u8; HXS], secs: f64) -> String {
    let rates = hash_to_rates(hx, secs);
    rates_to_show(rates)
}

pub fn u32_to_rateshow(num: u32, secs: f64) -> String {
    let rates = u32_to_rates(num, secs);
    rates_to_show(rates)
}

pub fn u32_to_rates(num: u32, secs: f64) -> f64 {
    let hx = u32_to_hash(num);
    hash_to_rates(&hx, secs)
}

pub fn hash_to_rates(hx: &[u8; HXS], secs: f64) -> f64 {
    if secs <= 0.0 || !secs.is_finite() {
        return 0.0
    }
    hash_to_power(&hx) / secs
}

pub fn hash_to_power_u128(hx: &[u8; HXS]) -> u128 {
    let power = hash_to_power(hx);
    power as u128
}

pub fn hash_to_power(hx: &[u8; HXS]) -> f64 {
    let target = BigUint::from_bytes_be(&hx[..]);
    if target == BigUint::ZERO {
        return 0.0
    }
    let numerator = BigUint::from(1u8) << (HXS * 8);
    let denominator = target + BigUint::from(1u8);
    let power = numerator.to_f64().unwrap() / denominator.to_f64().unwrap();
    if power.is_finite() {
        power
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_to_show_uses_standard_units() {
        assert_eq!(rates_to_show(0.0), "0.00H/s");
        assert_eq!(rates_to_show(-1.0), "0.00H/s");
        assert_eq!(rates_to_show(f64::NAN), "0.00H/s");
        assert_eq!(rates_to_show(f64::INFINITY), "0.00H/s");
        assert_eq!(rates_to_show(999.99), "999.99H/s");
        assert_eq!(rates_to_show(1000.0), "1.00KH/s");
        assert_eq!(rates_to_show(1500.0), "1.50KH/s");
        assert_eq!(rates_to_show(1_000_000.0), "1.00MH/s");
    }

    #[test]
    fn hash_to_power_matches_expected_boundary_values() {
        assert_eq!(hash_to_power(&[0; HXS]), 0.0);
        assert_eq!(hash_to_power(&[255; HXS]), 1.0);

        let mut target = [0u8; HXS];
        target[HXS - 1] = 1;
        let power = hash_to_power(&target);
        assert!(power.is_finite());
        assert!(power > 1.0e76);
    }

    #[test]
    fn u32_lowest_difficulty_is_about_one_hash_per_block_target() {
        let power = u32_to_rates(LOWEST_DIFFICULTY, 1.0);
        assert!(power > 1.0);
        assert!(power < 1.01);
    }
}





