use basis::interface::Action;

pub type ActCreateRes = Ret<Option<(Box<dyn Action>, usize)>>;
pub type ActCreateFun = fn(u16, &[u8]) -> ActCreateRes;

pub type ActJSONDecodeRes = Ret<Option<Box<dyn Action>>>;
pub type ActJSONDecodeFun = fn(u16, &str) -> ActJSONDecodeRes;

#[derive(Clone, Copy)]
pub struct ActionCodec {
    pub create: ActCreateFun,
    pub json_decode: ActJSONDecodeFun,
}

#[derive(Clone, Copy)]
pub struct ActionRegisterItem {
    pub kinds: &'static [u16],
    pub codec: ActionCodec,
}

impl ActionRegisterItem {
    pub const fn new(
        kinds: &'static [u16],
        create: ActCreateFun,
        json_decode: ActJSONDecodeFun,
    ) -> Self {
        Self {
            kinds,
            codec: ActionCodec {
                create,
                json_decode,
            },
        }
    }
}

pub fn do_action_create(kind: u16, buf: &[u8]) -> Ret<(Box<dyn Action>, usize)> {
    let registry = current_setup()?;
    let Some(codec) = registry.action_codecs.get(&kind).copied() else {
        return errf!("action kind {} not found", kind);
    };
    if let Some(act) = (codec.create)(kind, buf)? {
        return Ok(act);
    }
    errf!("action kind {} create failed", kind)
}

pub fn do_action_json_decode(kind: u16, json: &str) -> ActJSONDecodeRes {
    let registry = current_setup()?;
    let Some(codec) = registry.action_codecs.get(&kind).copied() else {
        return Ok(None);
    };
    if let Some(act) = (codec.json_decode)(kind, json)? {
        return Ok(Some(act));
    }
    errf!("action kind {} json decode failed", kind)
}
