
#[macro_export]
macro_rules! bufcut {
    ( $buf:expr, $l:expr, $r:expr ) => { 
        $buf[$l..$r].try_into().unwrap()
    };
}


/*
*
*/
fn ensure_buf_len(buflen: usize, n: usize) -> Ret<()> {
    maybe!(n > buflen, {
        let n1 = n.to_string();
        let n2 = buflen.to_string();
        Err("buf length too short: expected ".to_owned()+&n1+" but got "+&n2)
    }, Ok(()))
}

pub fn bufeat(buf: &[u8], n: usize) -> Ret<Vec<u8>> {
    ensure_buf_len(buf.len(), n)?;
    Ok(buf[..n].to_vec())
}

pub fn bufeat_ref(buf: &[u8], n: usize) -> Ret<&[u8]> {
    ensure_buf_len(buf.len(), n)?;
    Ok(&buf[..n])
}


/*
* 
*/
pub fn bufeatone(buf: &[u8]) -> Ret<u8> {
    maybe!(!buf.is_empty(), Ok(buf[0]), Err(s!("buf length too short")))
}
