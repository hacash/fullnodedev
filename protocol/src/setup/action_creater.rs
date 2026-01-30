use basis::interface::Action;



pub type ActCreateRes = Ret<Option<(Box<dyn Action>, usize)>>;
pub type ActCreateFun = fn(u16, &[u8]) -> ActCreateRes;

pub type ActJSONDecodeRes = Ret<Option<Box<dyn Action>>>;
pub type ActJSONDecodeFun = fn(u16, &str) -> ActJSONDecodeRes;


static mut ACTION_CREATE_LIST: OnceLock<Vec<ActCreateFun>> = OnceLock::new();
static mut ACTION_JSON_DECODE_LIST: OnceLock<Vec<ActJSONDecodeFun>> = OnceLock::new();

static mut ACTION_CREATE_LEN: usize = 0;
static mut ACTION_CREATE_PTR: &[ActCreateFun] = &[];
static mut ACTION_JSON_DECODE_PTR: &[ActJSONDecodeFun] = &[];
static mut ACTION_JSON_DECODE_LEN: usize = 0;


#[allow(static_mut_refs)]
pub fn action_register(create_fn: ActCreateFun, json_decode_fn: ActJSONDecodeFun) {
    unsafe {
        ACTION_CREATE_LIST.get_or_init(||vec![]);
        let list = ACTION_CREATE_LIST.get_mut().unwrap();
        list.push(create_fn);
        // get ptr
        ACTION_CREATE_LEN = list.len();
        ACTION_CREATE_PTR = list.as_slice();
        // json
        ACTION_JSON_DECODE_LIST.get_or_init(||vec![]);
        let list = ACTION_JSON_DECODE_LIST.get_mut().unwrap();
        list.push(json_decode_fn);
        ACTION_JSON_DECODE_LEN = list.len();
        ACTION_JSON_DECODE_PTR = list.as_slice();
    }
}



pub fn do_action_create(kind: u16, buf: &[u8]) -> Ret<(Box<dyn Action>, usize)> {
    unsafe {
        for idx in 0 .. ACTION_CREATE_LEN {
            if let Some(act) = ACTION_CREATE_PTR[idx](kind, buf)? {
                return Ok(act)
            }
        }
        errf!("action kind {} not find", kind)
    }
}


pub fn do_action_json_decode(kind: u16, json: &str) -> ActJSONDecodeRes {
    unsafe {
        for idx in 0 .. ACTION_JSON_DECODE_LEN {
            if let Some(act) = ACTION_JSON_DECODE_PTR[idx](kind, json)? {
                return Ok(Some(act))
            }
        }
        Ok(None)
    }
}
