

action_define!{TxMessage, 0x0401, 
    ActLv::Guard, // level
    false, [],
    {
        data:    BytesW1
    },
    (self, "Transaction message".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}


action_define!{TxBlob, 0x0402, 
    ActLv::Guard, // level
    false, [],
    {
        data:    BytesW2
    },
    (self, "Transaction blob data".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}




