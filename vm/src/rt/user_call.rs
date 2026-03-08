pub const CALL_BODY_WIDTH: usize = 8;
pub const TAILCALL_BODY_WIDTH: usize = 8;
pub const CALLCODE_BODY_WIDTH: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    Internal,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupBase {
    State,
    Code,
    Lib(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupWalk {
    Exact,
    Chain,
    Parents,
    Parent(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookupSpec {
    pub base: LookupBase,
    pub walk: LookupWalk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectShift {
    Inherit,
    ToView,
    ToPure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnBind {
    Callee,
    Caller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallExec {
    Enter(EffectMode),
    Jump {
        effect_shift: EffectShift,
        ret_bind: ReturnBind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSpec {
    pub boundary: Boundary,
    pub lookup: LookupSpec,
    pub selector: FnSign,
    pub exec: CallExec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTarget {
    This,
    Self_,
    Super,
    State,
    Code,
    Parent(u8),
    CurrentLibRoot(u8),
    CurrentLibChain(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallInvoke {
    Call(CallMode),
    Tail(ReturnBind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserCall {
    pub selector: FnSign,
    pub target: CallTarget,
    pub invoke: CallInvoke,
}

pub const fn lookup_spec(base: LookupBase, walk: LookupWalk) -> LookupSpec {
    LookupSpec { base, walk }
}

pub const fn lookup_for_target(target: CallTarget) -> LookupSpec {
    match target {
        CallTarget::This => lookup_spec(LookupBase::State, LookupWalk::Chain),
        CallTarget::Self_ => lookup_spec(LookupBase::Code, LookupWalk::Chain),
        CallTarget::Super => lookup_spec(LookupBase::Code, LookupWalk::Parents),
        CallTarget::CurrentLibChain(idx) => lookup_spec(LookupBase::Lib(idx), LookupWalk::Chain),
        CallTarget::State => lookup_spec(LookupBase::State, LookupWalk::Exact),
        CallTarget::Code => lookup_spec(LookupBase::Code, LookupWalk::Exact),
        CallTarget::Parent(idx) => lookup_spec(LookupBase::Code, LookupWalk::Parent(idx)),
        CallTarget::CurrentLibRoot(idx) => lookup_spec(LookupBase::Lib(idx), LookupWalk::Exact),
    }
}

pub const fn apply_effect_shift(current: EffectMode, shift: EffectShift) -> EffectMode {
    match shift {
        EffectShift::Inherit => current,
        EffectShift::ToView => EffectMode::View,
        EffectShift::ToPure => EffectMode::Pure,
    }
}

impl CallSpec {
    pub const fn requires_external_visibility(&self) -> bool {
        matches!(self.boundary, Boundary::External)
    }

    pub const fn reuses_current_frame(&self) -> bool {
        matches!(self.exec, CallExec::Jump { .. })
    }

    pub const fn return_bind(&self) -> Option<ReturnBind> {
        match self.exec {
            CallExec::Enter(_) => None,
            CallExec::Jump { ret_bind, .. } => Some(ret_bind),
        }
    }

    pub const fn next_effect(&self, current: EffectMode) -> EffectMode {
        match self.exec {
            CallExec::Enter(effect) => effect,
            CallExec::Jump { effect_shift, .. } => apply_effect_shift(current, effect_shift),
        }
    }

}

impl UserCall {
    pub const fn call(mode: CallMode, target: CallTarget, selector: FnSign) -> Self {
        Self {
            selector,
            target,
            invoke: CallInvoke::Call(mode),
        }
    }

    pub const fn tailcall(target: CallTarget, selector: FnSign) -> Self {
        Self::tailcall_with_ret(target, ReturnBind::Callee, selector)
    }

    pub const fn tailcall_with_ret(target: CallTarget, ret: ReturnBind, selector: FnSign) -> Self {
        Self {
            selector,
            target,
            invoke: CallInvoke::Tail(ret),
        }
    }

    pub const fn callcode(libidx: u8, selector: FnSign) -> Self {
        Self::tailcall_with_ret(
            CallTarget::CurrentLibRoot(libidx),
            ReturnBind::Caller,
            selector,
        )
    }

    pub const fn callext(libidx: u8, selector: FnSign) -> Self {
        Self::call(
            CallMode::External,
            CallTarget::CurrentLibChain(libidx),
            selector,
        )
    }

    pub const fn callview(libidx: u8, selector: FnSign) -> Self {
        Self::call(
            CallMode::View,
            CallTarget::CurrentLibChain(libidx),
            selector,
        )
    }

    pub const fn callpure(libidx: u8, selector: FnSign) -> Self {
        Self::call(
            CallMode::Pure,
            CallTarget::CurrentLibChain(libidx),
            selector,
        )
    }

    pub const fn callthis(selector: FnSign) -> Self {
        Self::call(CallMode::Inner, CallTarget::This, selector)
    }

    pub const fn callself(selector: FnSign) -> Self {
        Self::call(CallMode::Inner, CallTarget::Self_, selector)
    }

    pub const fn callsuper(selector: FnSign) -> Self {
        Self::call(CallMode::Inner, CallTarget::Super, selector)
    }

    pub const fn callselfview(selector: FnSign) -> Self {
        Self::call(CallMode::View, CallTarget::Self_, selector)
    }

    pub const fn callselfpure(selector: FnSign) -> Self {
        Self::call(CallMode::Pure, CallTarget::Self_, selector)
    }

    pub const fn to_spec(&self) -> CallSpec {
        let lookup = lookup_for_target(self.target);
        let selector = self.selector;
        match self.invoke {
            CallInvoke::Call(mode) => CallSpec {
                boundary: match mode {
                    CallMode::External => Boundary::External,
                    CallMode::Inner | CallMode::View | CallMode::Pure => Boundary::Internal,
                },
                lookup,
                selector,
                exec: CallExec::Enter(mode.to_effect()),
            },
            CallInvoke::Tail(ret_bind) => CallSpec {
                boundary: Boundary::Internal,
                lookup,
                selector,
                exec: CallExec::Jump {
                    effect_shift: EffectShift::Inherit,
                    ret_bind,
                },
            },
        }
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
            CallTarget::Super
        }
        3 => CallTarget::CurrentLibChain(arg),
        4 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::State
        }
        5 => {
            if arg != 0 {
                return itr_err_code!(CallInvalid);
            }
            CallTarget::Code
        }
        6 => CallTarget::Parent(arg),
        7 => CallTarget::CurrentLibRoot(arg),
        _ => return itr_err_code!(CallInvalid),
    })
}

fn encode_call_target(target: &CallTarget) -> (u8, u8) {
    match *target {
        CallTarget::This => (0, 0),
        CallTarget::Self_ => (1, 0),
        CallTarget::Super => (2, 0),
        CallTarget::CurrentLibChain(idx) => (3, idx),
        CallTarget::State => (4, 0),
        CallTarget::Code => (5, 0),
        CallTarget::Parent(idx) => (6, idx),
        CallTarget::CurrentLibRoot(idx) => (7, idx),
    }
}

pub fn verify_call(call: &UserCall) -> VmrtErr {
    if !matches!(call.invoke, CallInvoke::Call(_)) {
        return itr_err_code!(CallInvalid);
    }
    let _ = encode_call_target(&call.target);
    Ok(())
}

pub fn verify_tailcall(call: &UserCall) -> VmrtErr {
    if !matches!(call.invoke, CallInvoke::Tail(ReturnBind::Callee)) {
        return itr_err_code!(CallInvalid);
    }
    let _ = encode_call_target(&call.target);
    Ok(())
}

fn verify_callcode(call: &UserCall) -> VmrtErr {
    if !matches!(call.invoke, CallInvoke::Tail(ReturnBind::Caller))
        || !matches!(call.target, CallTarget::CurrentLibRoot(_))
    {
        return itr_err_code!(CallInvalid);
    }
    Ok(())
}

pub fn decode_call_body(s: &[u8]) -> VmrtRes<UserCall> {
    if s.len() != CALL_BODY_WIDTH {
        return itr_err!(CastParamFail, "call body size error");
    }
    if s[0] != 0 {
        return itr_err_code!(CallInvalid);
    }
    let flags = s[1];
    if flags & 0b0011_1111 != 0 {
        return itr_err_code!(CallInvalid);
    }
    let mode = match flags >> 6 {
        0 => CallMode::External,
        1 => CallMode::Inner,
        2 => CallMode::View,
        3 => CallMode::Pure,
        _ => unreachable!(),
    };
    let call = UserCall::call(
        mode,
        decode_call_target(s[2], s[3])?,
        checked_func_sign(&s[4..8])?,
    );
    verify_call(&call)?;
    Ok(call)
}

pub fn encode_call_body(call: UserCall) -> VmrtRes<[u8; CALL_BODY_WIDTH]> {
    verify_call(&call)?;
    let mut out = [0u8; CALL_BODY_WIDTH];
    out[0] = 0;
    out[1] = match call.invoke {
        CallInvoke::Call(CallMode::External) => 0b0000_0000,
        CallInvoke::Call(CallMode::Inner) => 0b0100_0000,
        CallInvoke::Call(CallMode::View) => 0b1000_0000,
        CallInvoke::Call(CallMode::Pure) => 0b1100_0000,
        CallInvoke::Tail(_) => return itr_err_code!(CallInvalid),
    };
    let (kind, arg) = encode_call_target(&call.target);
    out[2] = kind;
    out[3] = arg;
    out[4..8].copy_from_slice(&call.selector);
    Ok(out)
}

pub fn decode_tailcall_body(s: &[u8]) -> VmrtRes<UserCall> {
    if s.len() != TAILCALL_BODY_WIDTH {
        return itr_err!(CastParamFail, "tailcall body size error");
    }
    if s[0] != 0 || s[1] != 0 {
        return itr_err_code!(CallInvalid);
    }
    let call = UserCall::tailcall(
        decode_call_target(s[2], s[3])?,
        checked_func_sign(&s[4..8])?,
    );
    verify_tailcall(&call)?;
    Ok(call)
}

pub fn encode_tailcall_body(call: UserCall) -> VmrtRes<[u8; TAILCALL_BODY_WIDTH]> {
    verify_tailcall(&call)?;
    let mut out = [0u8; TAILCALL_BODY_WIDTH];
    out[0] = 0;
    out[1] = 0;
    let (kind, arg) = encode_call_target(&call.target);
    out[2] = kind;
    out[3] = arg;
    out[4..8].copy_from_slice(&call.selector);
    Ok(out)
}

pub fn decode_callcode_body(s: &[u8]) -> VmrtRes<UserCall> {
    if s.len() != CALLCODE_BODY_WIDTH {
        return itr_err!(CastParamFail, "callcode body size error");
    }
    let call = UserCall::callcode(s[0], checked_func_sign(&s[1..])?);
    verify_callcode(&call)?;
    Ok(call)
}

pub fn encode_callcode_body(call: UserCall) -> VmrtRes<[u8; CALLCODE_BODY_WIDTH]> {
    verify_callcode(&call)?;
    let CallTarget::CurrentLibRoot(idx) = call.target else {
        return itr_err_code!(CallInvalid);
    };
    let mut out = [0u8; CALLCODE_BODY_WIDTH];
    out[0] = idx;
    out[1..].copy_from_slice(&call.selector);
    Ok(out)
}

#[cfg(test)]
mod user_call_codec_tests {
    use super::*;

    fn sign() -> FnSign {
        [1, 2, 3, 4]
    }

    #[test]
    fn call_roundtrip_external_libchain() {
        let call = UserCall::call(CallMode::External, CallTarget::CurrentLibChain(7), sign());
        let body = encode_call_body(call).unwrap();
        assert_eq!(decode_call_body(&body).unwrap(), call);
        assert_eq!(call.to_spec().boundary, Boundary::External);
    }

    #[test]
    fn call_roundtrip_inner_code_exact() {
        let call = UserCall::call(CallMode::Inner, CallTarget::Code, sign());
        let body = encode_call_body(call).unwrap();
        assert_eq!(decode_call_body(&body).unwrap(), call);
        assert_eq!(
            call.to_spec().lookup,
            lookup_spec(LookupBase::Code, LookupWalk::Exact)
        );
    }

    #[test]
    fn tailcall_roundtrip_parent_exact() {
        let call = UserCall::tailcall(CallTarget::Parent(2), sign());
        let body = encode_tailcall_body(call).unwrap();
        assert_eq!(decode_tailcall_body(&body).unwrap(), call);
        assert!(call.to_spec().reuses_current_frame());
    }

    #[test]
    fn callcode_roundtrip_current_lib_root() {
        let call = UserCall::callcode(3, sign());
        let body = encode_callcode_body(call).unwrap();
        assert_eq!(decode_callcode_body(&body).unwrap(), call);
        assert_eq!(
            call.to_spec(),
            CallSpec {
                boundary: Boundary::Internal,
                lookup: lookup_spec(LookupBase::Lib(3), LookupWalk::Exact),
                selector: sign(),
                exec: CallExec::Jump {
                    effect_shift: EffectShift::Inherit,
                    ret_bind: ReturnBind::Caller,
                },
            }
        );
    }
}
