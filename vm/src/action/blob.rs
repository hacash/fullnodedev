

action_define!{TxMessage, 96, 
    ActLv::TopUnique, // level
    false, [],
    {
        data:    BytesW1
    },
    (self, "Transaction message".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}


action_define!{TxBlob, 97, 
    ActLv::TopUnique, // level
    false, [],
    {
        data:    BytesW2
    },
    (self, "Transaction blob data".to_owned()),
    (self, ctx, gas {
        Ok(vec![])
    })
}




