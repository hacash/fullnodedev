

pub trait ActExec {
    fn execute(&self, _: &mut dyn Context) -> Ret<(u32, Vec<u8>)> { never!() }
}


pub trait Description {
    fn to_description(&self) -> String { "".to_owned() }
}


pub trait Action : ActExec + Field + Description + Send + Sync + DynClone + std::fmt::Debug {
    fn kind(&self) -> u16 { never!() }
    fn level(&self) -> ActLv { ActLv::Top }
    fn burn_90(&self) -> bool { false } // is_burning_90_persent_fee
    fn req_sign(&self) -> Vec<AddrOrPtr> { vec![] } // request_need_sign_addresses

    fn as_any(&self) -> &dyn Any { never!() }
}

clone_trait_object!(Action);
