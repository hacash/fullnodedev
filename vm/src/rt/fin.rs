#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinRoundPolicy {
    Exact,
    Floor,
    Ceil,
    HalfUp,
    HalfEven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinKernel {
    Div,
    MulDiv,
    SqrtMul,
    Quantize,
    DevScaled,
    ScaledDiv,
    ScaledAdd,
    ScaledSub,
    MulShr,
    MulDivDenAdd,
    MulDivDenSub,
    MulAddDiv,
    MulSubDiv,
    Mul3Div,
    Wavg2,
    Lerp,
    AbsDiffLte,
    WithinBps,
    CrossLte,
    CrossGte,
    CrossEq,
    RPow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinSpec {
    pub family: Bytecode,
    pub id: u8,
    pub name: &'static str,
    pub kernel: FinKernel,
    pub round: Option<FinRoundPolicy>,
}

impl FinSpec {
    #[inline(always)]
    pub fn argc(self) -> VmrtRes<u8> {
        fin_family_argc(self.family)
    }

    #[inline(always)]
    pub fn round_or_exact(self) -> FinRoundPolicy {
        self.round.unwrap_or(FinRoundPolicy::Exact)
    }
}

macro_rules! fin_spec {
    ($family:expr, $id:literal, $name:literal, $kernel:expr, $round:expr) => {
        FinSpec {
            family: $family,
            id: $id,
            name: $name,
            kernel: $kernel,
            round: Some($round),
        }
    };
    ($family:expr, $id:literal, $name:literal, $kernel:expr) => {
        FinSpec {
            family: $family,
            id: $id,
            name: $name,
            kernel: $kernel,
            round: None,
        }
    };
}

const FIN_SPECS: &[FinSpec] = &[
    fin_spec!(Bytecode::FIN2, 0, "div_exact", FinKernel::Div, FinRoundPolicy::Exact),
    fin_spec!(Bytecode::FIN2, 1, "div_floor", FinKernel::Div, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN2, 2, "div_ceil", FinKernel::Div, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN2, 3, "div_half_up", FinKernel::Div, FinRoundPolicy::HalfUp),
    fin_spec!(Bytecode::FIN2, 4, "div_half_even", FinKernel::Div, FinRoundPolicy::HalfEven),
    fin_spec!(Bytecode::FIN2, 5, "sqrt_mul_floor", FinKernel::SqrtMul, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN2, 6, "sqrt_mul_ceil", FinKernel::SqrtMul, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN2, 7, "quantize_floor", FinKernel::Quantize, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN2, 8, "quantize_ceil", FinKernel::Quantize, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN3, 0, "mul_div_exact", FinKernel::MulDiv, FinRoundPolicy::Exact),
    fin_spec!(Bytecode::FIN3, 1, "mul_div_floor", FinKernel::MulDiv, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 2, "mul_div_ceil", FinKernel::MulDiv, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN3, 3, "mul_div_half_up", FinKernel::MulDiv, FinRoundPolicy::HalfUp),
    fin_spec!(Bytecode::FIN3, 4, "mul_div_half_even", FinKernel::MulDiv, FinRoundPolicy::HalfEven),
    fin_spec!(Bytecode::FIN3, 5, "dev_scaled_floor", FinKernel::DevScaled, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 6, "dev_scaled_ceil", FinKernel::DevScaled, FinRoundPolicy::Ceil),
    fin_spec!(
        Bytecode::FIN3,
        7,
        "dev_scaled_half_even",
        FinKernel::DevScaled,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(Bytecode::FIN3, 8, "scaled_div_floor", FinKernel::ScaledDiv, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 9, "scaled_div_ceil", FinKernel::ScaledDiv, FinRoundPolicy::Ceil),
    fin_spec!(
        Bytecode::FIN3,
        10,
        "scaled_div_half_up",
        FinKernel::ScaledDiv,
        FinRoundPolicy::HalfUp
    ),
    fin_spec!(
        Bytecode::FIN3,
        11,
        "scaled_div_half_even",
        FinKernel::ScaledDiv,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(Bytecode::FIN3, 12, "mul_shr_floor", FinKernel::MulShr, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 13, "mul_shr_ceil", FinKernel::MulShr, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN3, 14, "scaled_add_floor", FinKernel::ScaledAdd, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 15, "scaled_add_ceil", FinKernel::ScaledAdd, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN3, 16, "scaled_sub_floor", FinKernel::ScaledSub, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN3, 17, "scaled_sub_ceil", FinKernel::ScaledSub, FinRoundPolicy::Ceil),
    fin_spec!(
        Bytecode::FIN3,
        18,
        "mul_div_den_add_floor",
        FinKernel::MulDivDenAdd,
        FinRoundPolicy::Floor
    ),
    fin_spec!(
        Bytecode::FIN3,
        19,
        "mul_div_den_add_ceil",
        FinKernel::MulDivDenAdd,
        FinRoundPolicy::Ceil
    ),
    fin_spec!(
        Bytecode::FIN3,
        20,
        "mul_div_den_sub_floor",
        FinKernel::MulDivDenSub,
        FinRoundPolicy::Floor
    ),
    fin_spec!(
        Bytecode::FIN3,
        21,
        "mul_div_den_sub_ceil",
        FinKernel::MulDivDenSub,
        FinRoundPolicy::Ceil
    ),
    fin_spec!(
        Bytecode::FIN4,
        0,
        "mul_add_div_exact",
        FinKernel::MulAddDiv,
        FinRoundPolicy::Exact
    ),
    fin_spec!(
        Bytecode::FIN4,
        1,
        "mul_add_div_floor",
        FinKernel::MulAddDiv,
        FinRoundPolicy::Floor
    ),
    fin_spec!(
        Bytecode::FIN4,
        2,
        "mul_add_div_ceil",
        FinKernel::MulAddDiv,
        FinRoundPolicy::Ceil
    ),
    fin_spec!(
        Bytecode::FIN4,
        3,
        "mul_add_div_half_up",
        FinKernel::MulAddDiv,
        FinRoundPolicy::HalfUp
    ),
    fin_spec!(
        Bytecode::FIN4,
        4,
        "mul_add_div_half_even",
        FinKernel::MulAddDiv,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(
        Bytecode::FIN4,
        5,
        "mul_sub_div_exact",
        FinKernel::MulSubDiv,
        FinRoundPolicy::Exact
    ),
    fin_spec!(
        Bytecode::FIN4,
        6,
        "mul_sub_div_floor",
        FinKernel::MulSubDiv,
        FinRoundPolicy::Floor
    ),
    fin_spec!(
        Bytecode::FIN4,
        7,
        "mul_sub_div_ceil",
        FinKernel::MulSubDiv,
        FinRoundPolicy::Ceil
    ),
    fin_spec!(
        Bytecode::FIN4,
        8,
        "mul_sub_div_half_up",
        FinKernel::MulSubDiv,
        FinRoundPolicy::HalfUp
    ),
    fin_spec!(
        Bytecode::FIN4,
        9,
        "mul_sub_div_half_even",
        FinKernel::MulSubDiv,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(Bytecode::FIN4, 10, "mul3_div_exact", FinKernel::Mul3Div, FinRoundPolicy::Exact),
    fin_spec!(Bytecode::FIN4, 11, "mul3_div_floor", FinKernel::Mul3Div, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN4, 12, "mul3_div_ceil", FinKernel::Mul3Div, FinRoundPolicy::Ceil),
    fin_spec!(
        Bytecode::FIN4,
        13,
        "mul3_div_half_up",
        FinKernel::Mul3Div,
        FinRoundPolicy::HalfUp
    ),
    fin_spec!(
        Bytecode::FIN4,
        14,
        "mul3_div_half_even",
        FinKernel::Mul3Div,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(Bytecode::FIN4, 15, "wavg2_exact", FinKernel::Wavg2, FinRoundPolicy::Exact),
    fin_spec!(Bytecode::FIN4, 16, "wavg2_floor", FinKernel::Wavg2, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN4, 17, "wavg2_ceil", FinKernel::Wavg2, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN4, 18, "wavg2_half_up", FinKernel::Wavg2, FinRoundPolicy::HalfUp),
    fin_spec!(
        Bytecode::FIN4,
        19,
        "wavg2_half_even",
        FinKernel::Wavg2,
        FinRoundPolicy::HalfEven
    ),
    fin_spec!(Bytecode::FIN4, 20, "lerp_exact", FinKernel::Lerp, FinRoundPolicy::Exact),
    fin_spec!(Bytecode::FIN4, 21, "lerp_floor", FinKernel::Lerp, FinRoundPolicy::Floor),
    fin_spec!(Bytecode::FIN4, 22, "lerp_ceil", FinKernel::Lerp, FinRoundPolicy::Ceil),
    fin_spec!(Bytecode::FIN4, 23, "lerp_half_up", FinKernel::Lerp, FinRoundPolicy::HalfUp),
    fin_spec!(Bytecode::FIN4, 24, "lerp_half_even", FinKernel::Lerp, FinRoundPolicy::HalfEven),
    fin_spec!(Bytecode::FINP3, 0, "abs_diff_lte", FinKernel::AbsDiffLte),
    fin_spec!(Bytecode::FINP4, 0, "within_bps", FinKernel::WithinBps),
    fin_spec!(Bytecode::FINP4, 1, "cross_lte", FinKernel::CrossLte),
    fin_spec!(Bytecode::FINP4, 2, "cross_gte", FinKernel::CrossGte),
    fin_spec!(Bytecode::FINP4, 3, "cross_eq", FinKernel::CrossEq),
    fin_spec!(Bytecode::FINPOW3, 0, "rpow_half_up", FinKernel::RPow, FinRoundPolicy::HalfUp),
];

#[inline(always)]
pub fn is_fin_family(family: Bytecode) -> bool {
    matches!(
        family,
        Bytecode::FIN2
            | Bytecode::FIN3
            | Bytecode::FIN4
            | Bytecode::FINP3
            | Bytecode::FINP4
            | Bytecode::FINPOW3
    )
}

pub fn fin_family_argc(family: Bytecode) -> VmrtRes<u8> {
    match family {
        Bytecode::FIN2 => Ok(2),
        Bytecode::FIN3 | Bytecode::FINP3 | Bytecode::FINPOW3 => Ok(3),
        Bytecode::FIN4 | Bytecode::FINP4 => Ok(4),
        _ => itr_err_fmt!(InstParamsErr, "not a fin family opcode: {:?}", family),
    }
}

pub fn fin_spec_lookup(family: Bytecode, id: u8) -> VmrtRes<FinSpec> {
    fin_specs()
        .iter()
        .copied()
        .find(|spec| spec.family == family && spec.id == id)
        .ok_or_else(|| {
            ItrErr::new(
                InstParamsErr,
                &format!("unknown fin registry entry {:?}/{}", family, id),
            )
        })
}

pub fn fin_specs() -> &'static [FinSpec] {
    FIN_SPECS
}

pub fn fin_source_call_spec(id: &str) -> VmrtRes<Option<FinSpec>> {
    Ok(fin_specs().iter().copied().find(|spec| spec.name == id))
}

pub fn verify_fin_runtime_supported(family: Bytecode, id: u8) -> VmrtErr {
    let _ = fin_spec_lookup(family, id)?;
    Ok(())
}

#[cfg(test)]
mod fin_source_name_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn fin_source_name_table_is_unique_and_runtime_supported() {
        let mut names = HashSet::new();
        for spec in fin_specs() {
            assert!(names.insert(spec.name), "duplicate FIN source name {}", spec.name);
            assert_eq!(
                fin_source_call_spec(spec.name).unwrap(),
                Some(*spec)
            );
            assert_eq!(fin_spec_lookup(spec.family, spec.id).unwrap(), *spec);
        }
    }

    #[test]
    fn fin_source_names_do_not_shadow_direct_ir_helpers() {
        for spec in fin_specs() {
            assert!(
                pick_ir_func(spec.name).is_none(),
                "FIN source name {} shadows a direct IR helper",
                spec.name
            );
        }
    }

    #[test]
    fn fin_registry_family_ids_are_unique() {
        let mut keys = HashSet::new();
        for spec in fin_specs() {
            assert!(
                keys.insert((spec.family as u8, spec.id)),
                "duplicate (family, id) in FIN_SPECS: {:?}/{}",
                spec.family,
                spec.id
            );
        }
    }

    #[test]
    fn fin_registry_family_ids_are_contiguous() {
        let families = [
            Bytecode::FIN2,
            Bytecode::FIN3,
            Bytecode::FIN4,
            Bytecode::FINP3,
            Bytecode::FINP4,
            Bytecode::FINPOW3,
        ];
        for family in families {
            let mut ids = fin_specs()
                .iter()
                .filter(|spec| spec.family == family)
                .map(|spec| spec.id)
                .collect::<Vec<_>>();
            ids.sort_unstable();
            for (expected, id) in ids.iter().enumerate() {
                assert_eq!(
                    *id, expected as u8,
                    "FIN ids for {:?} must be contiguous from 0: {:?}",
                    family, ids
                );
            }
        }
    }

    #[test]
    fn fin_spec_lookup_success_implies_registry_row() {
        let families = [
            Bytecode::FIN2,
            Bytecode::FIN3,
            Bytecode::FIN4,
            Bytecode::FINP3,
            Bytecode::FINP4,
            Bytecode::FINPOW3,
        ];
        let mut reg: HashSet<(u8, u8)> = fin_specs()
            .iter()
            .map(|s| (s.family as u8, s.id))
            .collect();
        for family in families {
            for id in 0u8..=u8::MAX {
                if fin_spec_lookup(family, id).is_ok() {
                    assert!(
                        reg.remove(&(family as u8, id)),
                        "fin_spec_lookup({:?}, {}) ok but no registry row",
                        family,
                        id
                    );
                }
            }
        }
        assert!(
            reg.is_empty(),
            "registry rows not reachable by any id scan: {:?}",
            reg
        );
    }
}
