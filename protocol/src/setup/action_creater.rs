use basis::interface::Action;



pub type ActCreateRes = Ret<Option<(Box<dyn Action>, usize)>>;
pub type ActCreateFun = fn(u16, &[u8]) -> ActCreateRes;

static mut ACTION_CREATE_LIST: OnceLock<Vec<ActCreateFun>> = OnceLock::new();

static mut ACTION_CREATE_LEN: usize = 0;
static mut ACTION_CREATE_PTR: &[ActCreateFun] = &[];

#[allow(static_mut_refs)]
pub fn action_register(create_fn: ActCreateFun) {
    unsafe {
        ACTION_CREATE_LIST.get_or_init(||vec![]);
        let list = ACTION_CREATE_LIST.get_mut().unwrap();
        list.push(create_fn);
        // get ptr
        ACTION_CREATE_LEN = list.len();
        ACTION_CREATE_PTR = list.as_slice();
    }
}



pub fn action_create(kind: u16, buf: &[u8]) -> Ret<(Box<dyn Action>, usize)> {
    unsafe {
        for idx in 0 .. ACTION_CREATE_LEN {
            if let Some(act) = ACTION_CREATE_PTR[idx](kind, buf)? {
                return Ok(act)
            }
        }
        errf!("action kind {} not find", kind)
    }
}