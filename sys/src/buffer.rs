
#[macro_export]
macro_rules! bufcut {
    ( $buf:expr, $l:expr, $r:expr ) => { 
        $buf[$l..$r].try_into().unwrap()
    };
}


/*
*
*/
pub fn bufeat(buf: &[u8], n: usize) -> Ret<Vec<u8>> {
    let buflen = buf.len();
    maybe!(n > buflen, {
        let n1 = n.to_string();
        let n2 = buflen.to_string();
        Err("buf length too short need ".to_owned()+&n1+" but got "+&n2)
    }, Ok(buf[..n].to_vec()))
}


/*
* 
*/
pub fn bufeatone(buf: &[u8]) -> Ret<u8> {
    maybe!(buf.len() >= 1, Ok(buf[0]), Err(s!("buf length too short")))
}

