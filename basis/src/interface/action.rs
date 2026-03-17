pub trait ActExec {
    fn execute(&self, _: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        never!()
    }
}

pub trait Description {
    fn to_description(&self) -> String {
        "".to_owned()
    }
}

pub trait Action: ActExec + Field + Description + Send + Sync + DynClone + std::fmt::Debug {
    fn kind(&self) -> u16 {
        never!()
    }
    fn scope(&self) -> ActScope {
        ActScope::TOP
    }
    fn min_tx_type(&self) -> u8 {
        1
    }
    fn extra9(&self) -> bool {
        false
    }
    fn req_sign(&self) -> Vec<AddrOrPtr> {
        vec![]
    } // request_need_sign_addresses

    fn as_any(&self) -> &dyn Any {
        never!()
    }
}

clone_trait_object!(Action);
