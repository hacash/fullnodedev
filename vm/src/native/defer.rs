// NTCTL.defer: Defer callback registration native function

/// Register current concrete contract for Defer callback
/// This is a VM-level primitive (NTCTL), not a protocol-level action
///
/// Parameters: none
///
/// Returns:
/// - nil: successfully registered or already registered
///
/// Errors:
/// - DeferredError: not in concrete contract context or not in collecting phase
pub fn defer(_hei: u64, v: &[u8]) -> VmrtRes<Value> {
    if !v.is_empty() {
        return itr_err_fmt!(NativeCtlError, "defer takes no parameters");
    }
    // Return nil to indicate success.
    // Context checks and registry mutation are handled by the interpreter.
    Ok(Value::Nil)
}
