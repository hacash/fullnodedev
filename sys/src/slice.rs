

pub fn cover<'a, T: Copy>(dst: &'a mut Vec<T>, src: &'a [T]) -> &'a mut Vec<T> {
    let mut ln = dst.len();
    let l2 = src.len();
    if l2 < ln {
        ln = l2;
    }
    // copy
    dst[..ln].copy_from_slice(&src[..ln]);
    dst
}

pub fn cover_clone<'a, T: Clone>(dst: &'a mut Vec<T>, src: &'a [T]) -> &'a mut Vec<T> {
    let mut ln = dst.len();
    let l2 = src.len();
    if l2 < ln {
        ln = l2;
    }
    // copy
    dst[..ln].clone_from_slice(&src[..ln]);
    dst
}
