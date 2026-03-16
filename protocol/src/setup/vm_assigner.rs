/*
    VM assigner: allows vm crate to register its assign function
    so that tx session can lazily initialize VM on first VM call.
*/

pub type FnVmAssignFunc = fn(height: u64) -> Box<dyn VM>;
