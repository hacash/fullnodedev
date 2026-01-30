
pub trait CellExec {
    // return same as ActExec
    fn execute(&self, _: &mut dyn Context, _: &Address) -> Rerr { never!() }
}


pub trait TexCell: Send + Sync + DynClone + Field + CellExec {
    fn kind(&self) -> u16;
}


clone_trait_object!{TexCell}

