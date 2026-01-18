

fn cut_kind(buf: &[u8]) -> Ret<u16> {
    let mut kind = Uint2::default();
    kind.parse(buf)?;
    Ok(*kind)
}


pub fn action_create(buf: &[u8]) -> Ret<(Box<dyn Action>, usize)> {
    let kid = cut_kind(buf)?;
    do_action_create(kid, buf)
}

/*
pub fn _create_old(buf: &[u8]) -> Ret<(Box<dyn Action>, usize)> {
    let kid = cut_kind(buf)?;
    unsafe {
        for create_action in EXTEND_ACTIONS_TRY_CREATE_FUNCS {
            let cres = create_action(kid, buf)?;
            match cres {
                Some(a) => return Ok(a),
                _ => continue, // next
            }
        }
    }
    // not find
    errf!("action kind '{}' not find", kid).to_owned()
}
*/

/*
* list defind
*/
combi_dynlist!{ DynListActionW1,
    Uint1, Action, action_create
}

combi_dynlist!{ DynListActionW2,
    Uint2, Action, action_create
}





