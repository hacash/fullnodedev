

action_define!{ TxMessage, 0x0401, 
    ActScope::GUARD, false, [],
    {
        data:    BytesW1
    },
    (self, "Transaction message".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}


action_define!{ TxBlob, 0x0402, 
    ActScope::GUARD, false, [],
    {
        data:    BytesW2
    },
    (self, "Transaction blob data".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}




