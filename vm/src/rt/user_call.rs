pub const CALL_BODY_WIDTH: usize = 6;
pub const SPLICE_BODY_WIDTH: usize = 5;
pub const USECODE_BODY_WIDTH: usize = SPLICE_BODY_WIDTH;

const CALL_TARGET_MASK: u8 = 0b0000_0111;
const CALL_EFFECT_MASK: u8 = 0b0001_1000;
const CALL_RESERVED_MASK: u8 = 0b1110_0000;
const SHORT_CALL_BODY_WIDTH: usize = FN_SIGN_WIDTH;
const SHORT_LIB_CALL_BODY_WIDTH: usize = 1 + FN_SIGN_WIDTH;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTarget {
    This,
    Self_,
    Upper,
    Super,
    Call(u8),
    Use(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallSpec {
    Invoke {
        target: CallTarget,
        effect: EffectMode,
        selector: FnSign,
    },
    Splice {
        lib: u8,
        selector: FnSign,
    },
}

impl CallTarget {
    pub const fn anchor_from_state(self) -> bool {
        matches!(self, Self::This)
    }

    pub const fn anchor_from_code(self) -> bool {
        matches!(self, Self::Self_ | Self::Upper | Self::Super)
    }

    pub const fn lib_index(self) -> Option<u8> {
        match self {
            Self::Call(idx) | Self::Use(idx) => Some(idx),
            _ => None,
        }
    }

    pub const fn searches_exact(self) -> bool {
        matches!(self, Self::Self_ | Self::Use(_))
    }

    pub const fn searches_parents(self) -> bool {
        matches!(self, Self::Super)
    }

    pub const fn switches_context(self) -> bool {
        matches!(self, Self::Call(_))
    }
}

impl CallSpec {
    pub const fn invoke(target: CallTarget, effect: EffectMode, selector: FnSign) -> Self {
        Self::Invoke {
            target,
            effect,
            selector,
        }
    }

    pub const fn splice(lib: u8, selector: FnSign) -> Self {
        Self::Splice { lib, selector }
    }

    pub const fn selector(&self) -> FnSign {
        match *self {
            Self::Invoke { selector, .. } | Self::Splice { selector, .. } => selector,
        }
    }

    pub const fn target(&self) -> CallTarget {
        match *self {
            Self::Invoke { target, .. } => target,
            Self::Splice { lib, .. } => CallTarget::Use(lib),
        }
    }

    pub const fn next_effect(&self, current: EffectMode) -> EffectMode {
        match *self {
            Self::Invoke { effect, .. } => effect,
            Self::Splice { .. } => current,
        }
    }

    pub const fn requires_external_visibility(&self) -> bool {
        matches!(
            *self,
            Self::Invoke {
                target: CallTarget::Call(_),
                effect: EffectMode::Edit,
                ..
            }
        )
    }

    pub const fn reuses_current_frame(&self) -> bool {
        matches!(self, Self::Splice { .. })
    }

    pub const fn switches_context(&self) -> bool {
        self.target().switches_context()
    }

    pub const fn callext(libidx: u8, selector: FnSign) -> Self {
        Self::invoke(CallTarget::Call(libidx), EffectMode::Edit, selector)
    }

    pub const fn callview(libidx: u8, selector: FnSign) -> Self {
        Self::invoke(CallTarget::Call(libidx), EffectMode::View, selector)
    }

    pub const fn callpure(libidx: u8, selector: FnSign) -> Self {
        Self::invoke(CallTarget::Call(libidx), EffectMode::Pure, selector)
    }

    pub const fn callthis(selector: FnSign) -> Self {
        Self::invoke(CallTarget::This, EffectMode::Edit, selector)
    }

    pub const fn callself(selector: FnSign) -> Self {
        Self::invoke(CallTarget::Self_, EffectMode::Edit, selector)
    }

    pub const fn callupper(selector: FnSign) -> Self {
        Self::invoke(CallTarget::Upper, EffectMode::Edit, selector)
    }

    pub const fn callsuper(selector: FnSign) -> Self {
        Self::invoke(CallTarget::Super, EffectMode::Edit, selector)
    }

    pub const fn callselfview(selector: FnSign) -> Self {
        Self::invoke(CallTarget::Self_, EffectMode::View, selector)
    }

    pub const fn callselfpure(selector: FnSign) -> Self {
        Self::invoke(CallTarget::Self_, EffectMode::Pure, selector)
    }

    pub const fn usecode(libidx: u8, selector: FnSign) -> Self {
        Self::splice(libidx, selector)
    }
}

fn decode_call_target(tag: u8, arg: u8) -> VmrtRes<CallTarget> {
    Ok(match tag {
        0 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::This
        }
        1 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::Self_
        }
        2 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::Upper
        }
        3 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::Super
        }
        4 => CallTarget::Call(arg),
        5 => CallTarget::Use(arg),
        _ => return itr_err_code!(CallInvalid),
    })
}

fn encode_call_target(target: CallTarget) -> (u8, u8) {
    match target {
        CallTarget::This => (0, 0),
        CallTarget::Self_ => (1, 0),
        CallTarget::Upper => (2, 0),
        CallTarget::Super => (3, 0),
        CallTarget::Call(idx) => (4, idx),
        CallTarget::Use(idx) => (5, idx),
    }
}

fn decode_short_call_body(s: &[u8], target: CallTarget, effect: EffectMode) -> VmrtRes<CallSpec> {
    if s.len() != SHORT_CALL_BODY_WIDTH {
        return itr_err!(CastParamFail, "call shortcut body size error");
    }
    Ok(CallSpec::invoke(target, effect, checked_func_sign(s)?))
}

fn decode_short_lib_call_body(s: &[u8], effect: EffectMode) -> VmrtRes<CallSpec> {
    if s.len() != SHORT_LIB_CALL_BODY_WIDTH {
        return itr_err!(CastParamFail, "call shortcut body size error");
    }
    Ok(CallSpec::invoke(
        CallTarget::Call(s[0]),
        effect,
        checked_func_sign(&s[1..])?,
    ))
}

fn encode_short_call_body(selector: FnSign) -> Vec<u8> {
    selector.to_vec()
}

fn encode_short_lib_call_body(lib: u8, selector: FnSign) -> Vec<u8> {
    let mut out = Vec::with_capacity(SHORT_LIB_CALL_BODY_WIDTH);
    out.push(lib);
    out.extend_from_slice(&selector);
    out
}

pub const fn is_user_call_inst(inst: Bytecode) -> bool {
    matches!(
        inst,
        Bytecode::USECODE
            | Bytecode::CALL
            | Bytecode::CALLTHIS
            | Bytecode::CALLSELF
            | Bytecode::CALLSUPER
            | Bytecode::CALLSELFVIEW
            | Bytecode::CALLSELFPURE
            | Bytecode::CALLEXT
            | Bytecode::CALLVIEW
            | Bytecode::CALLPURE
    )
}

pub fn decode_user_call_site(inst: Bytecode, s: &[u8]) -> VmrtRes<CallSpec> {
    match inst {
        Bytecode::USECODE => decode_usecode_body(s),
        Bytecode::CALL => decode_call_body(s),
        Bytecode::CALLTHIS => decode_short_call_body(s, CallTarget::This, EffectMode::Edit),
        Bytecode::CALLSELF => decode_short_call_body(s, CallTarget::Self_, EffectMode::Edit),
        Bytecode::CALLSUPER => decode_short_call_body(s, CallTarget::Super, EffectMode::Edit),
        Bytecode::CALLSELFVIEW => decode_short_call_body(s, CallTarget::Self_, EffectMode::View),
        Bytecode::CALLSELFPURE => decode_short_call_body(s, CallTarget::Self_, EffectMode::Pure),
        Bytecode::CALLEXT => decode_short_lib_call_body(s, EffectMode::Edit),
        Bytecode::CALLVIEW => decode_short_lib_call_body(s, EffectMode::View),
        Bytecode::CALLPURE => decode_short_lib_call_body(s, EffectMode::Pure),
        _ => itr_err_code!(CallInvalid),
    }
}

pub fn encode_user_call_site(call: CallSpec) -> VmrtRes<(Bytecode, Vec<u8>)> {
    Ok(match call {
        CallSpec::Splice { .. } => (Bytecode::USECODE, encode_usecode_body(call)?.to_vec()),
        CallSpec::Invoke {
            target,
            effect,
            selector,
        } => match (target, effect) {
            (CallTarget::This, EffectMode::Edit) => {
                (Bytecode::CALLTHIS, encode_short_call_body(selector))
            }
            (CallTarget::Self_, EffectMode::Edit) => {
                (Bytecode::CALLSELF, encode_short_call_body(selector))
            }
            (CallTarget::Super, EffectMode::Edit) => {
                (Bytecode::CALLSUPER, encode_short_call_body(selector))
            }
            (CallTarget::Self_, EffectMode::View) => {
                (Bytecode::CALLSELFVIEW, encode_short_call_body(selector))
            }
            (CallTarget::Self_, EffectMode::Pure) => {
                (Bytecode::CALLSELFPURE, encode_short_call_body(selector))
            }
            (CallTarget::Call(lib), EffectMode::Edit) => {
                (Bytecode::CALLEXT, encode_short_lib_call_body(lib, selector))
            }
            (CallTarget::Call(lib), EffectMode::View) => {
                (Bytecode::CALLVIEW, encode_short_lib_call_body(lib, selector))
            }
            (CallTarget::Call(lib), EffectMode::Pure) => {
                (Bytecode::CALLPURE, encode_short_lib_call_body(lib, selector))
            }
            _ => (Bytecode::CALL, encode_call_body(call)?.to_vec()),
        },
    })
}

pub fn verify_call(call: &CallSpec) -> VmrtErr {
    if !matches!(call, CallSpec::Invoke { .. }) {
        return itr_err_code!(CallInvalid);
    }
    Ok(())
}

pub fn decode_call_body(s: &[u8]) -> VmrtRes<CallSpec> {
    if s.len() != CALL_BODY_WIDTH {
        return itr_err!(CastParamFail, "call body size error");
    }
    let flags = s[0];
    if flags & CALL_RESERVED_MASK != 0 {
        return itr_err_code!(CallInvalid);
    }
    let effect = match (flags & CALL_EFFECT_MASK) >> 3 {
        0 => EffectMode::Edit,
        1 => EffectMode::View,
        2 => EffectMode::Pure,
        _ => return itr_err_code!(CallInvalid),
    };
    let call = CallSpec::invoke(
        decode_call_target(flags & CALL_TARGET_MASK, s[1])?,
        effect,
        checked_func_sign(&s[2..6])?,
    );
    verify_call(&call)?;
    Ok(call)
}

pub fn encode_call_body(call: CallSpec) -> VmrtRes<[u8; CALL_BODY_WIDTH]> {
    verify_call(&call)?;
    let CallSpec::Invoke {
        target,
        effect,
        selector,
    } = call
    else {
        return itr_err_code!(CallInvalid);
    };
    let (tag, arg) = encode_call_target(target);
    let mut out = [0u8; CALL_BODY_WIDTH];
    out[0] = tag
        | match effect {
            EffectMode::Edit => 0,
            EffectMode::View => 0b0000_1000,
            EffectMode::Pure => 0b0001_0000,
        };
    out[1] = arg;
    out[2..6].copy_from_slice(&selector);
    Ok(out)
}

pub fn verify_splice(call: &CallSpec) -> VmrtErr {
    if !matches!(call, CallSpec::Splice { .. }) {
        return itr_err_code!(CallInvalid);
    }
    Ok(())
}

pub fn decode_splice_body(s: &[u8]) -> VmrtRes<CallSpec> {
    if s.len() != SPLICE_BODY_WIDTH {
        return itr_err!(CastParamFail, "splice body size error");
    }
    let call = CallSpec::splice(s[0], checked_func_sign(&s[1..])?);
    verify_splice(&call)?;
    Ok(call)
}

pub fn encode_splice_body(call: CallSpec) -> VmrtRes<[u8; SPLICE_BODY_WIDTH]> {
    verify_splice(&call)?;
    let CallSpec::Splice { lib, selector } = call else {
        return itr_err_code!(CallInvalid);
    };
    let mut out = [0u8; SPLICE_BODY_WIDTH];
    out[0] = lib;
    out[1..].copy_from_slice(&selector);
    Ok(out)
}

pub fn verify_usecode(call: &CallSpec) -> VmrtErr {
    verify_splice(call)
}

pub fn decode_usecode_body(s: &[u8]) -> VmrtRes<CallSpec> {
    decode_splice_body(s)
}

pub fn encode_usecode_body(call: CallSpec) -> VmrtRes<[u8; USECODE_BODY_WIDTH]> {
    encode_splice_body(call)
}

#[cfg(test)]
mod user_call_codec_tests {
    use super::*;

    fn sign() -> FnSign {
        [1, 2, 3, 4]
    }

    #[test]
    fn call_roundtrip_external_lib_chain() {
        let call = CallSpec::callext(7, sign());
        let body = encode_call_body(call).unwrap();
        assert_eq!(decode_call_body(&body).unwrap(), call);
        assert!(call.requires_external_visibility());
        assert!(call.switches_context());
    }

    #[test]
    fn view_and_pure_lib_calls_do_not_require_external_visibility() {
        assert!(!CallSpec::callview(7, sign()).requires_external_visibility());
        assert!(!CallSpec::callpure(7, sign()).requires_external_visibility());
    }

    #[test]
    fn call_roundtrip_internal_use_exact() {
        let call = CallSpec::invoke(CallTarget::Use(3), EffectMode::View, sign());
        let body = encode_call_body(call).unwrap();
        assert_eq!(decode_call_body(&body).unwrap(), call);
        assert!(!call.switches_context());
    }

    #[test]
    fn call_roundtrip_upper_chain() {
        let call = CallSpec::callupper(sign());
        let body = encode_call_body(call).unwrap();
        assert_eq!(decode_call_body(&body).unwrap(), call);
        assert!(!call.switches_context());
    }

    #[test]
    fn splice_roundtrip_body() {
        let call = CallSpec::splice(3, sign());
        let body = encode_splice_body(call).unwrap();
        assert_eq!(decode_splice_body(&body).unwrap(), call);
        assert!(call.reuses_current_frame());
        assert!(!call.switches_context());
    }

    #[test]
    fn encode_user_call_site_prefers_shortcuts() {
        let call = CallSpec::callself(sign());
        let (inst, body) = encode_user_call_site(call).unwrap();
        assert_eq!(inst, Bytecode::CALLSELF);
        assert_eq!(decode_user_call_site(inst, &body).unwrap(), call);
    }

    #[test]
    fn encode_user_call_site_keeps_generic_when_needed() {
        let call = CallSpec::invoke(CallTarget::Upper, EffectMode::View, sign());
        let (inst, body) = encode_user_call_site(call).unwrap();
        assert_eq!(inst, Bytecode::CALL);
        assert_eq!(decode_user_call_site(inst, &body).unwrap(), call);
    }
}
