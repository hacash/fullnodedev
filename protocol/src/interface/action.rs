/*
*
*/
pub trait ActExec {
    // return: (more gas use, exec value)
    fn execute(&self, _: &mut dyn Context) -> Ret<(u32, Vec<u8>)> { unimplemented!() }
}


/*
*
*/
pub trait Action : ActExec + Field + Send + Sync + DynClone {
    fn kind(&self) -> u16 { unimplemented!() }
    fn level(&self) -> ActLv { ActLv::Top }
    fn burn_90(&self) -> bool { false } // is_burning_90_persent_fee
    fn req_sign(&self) -> Vec<AddrOrPtr> { vec![] } // request_need_sign_addresses

    fn as_any(&self) -> &dyn Any { unimplemented!() }
}

clone_trait_object!(Action);





