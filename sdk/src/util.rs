


#[wasm_bindgen]
pub fn hac_to_unit(stuff: &str, unit: u8) -> Ret<f64> {
    if unit > UNIT_MEI {
        return errf!("unit {} out of range, max {}", unit, UNIT_MEI);
    }
    Amount::from(stuff).map(|a| unsafe { a.to_unit_float(unit) })
}

#[wasm_bindgen]
pub fn hac_to_mei(stuff: &str) -> Ret<f64> {
    hac_to_unit(stuff, UNIT_MEI)
}
