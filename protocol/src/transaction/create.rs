



macro_rules! transaction_register {
    ( $( $tty:ident )+ ) => {
        
        pub fn transaction_create(buf: &[u8]) -> Ret<(Box<dyn Transaction>, usize)> {
            let ty = bufeatone(buf)?;
            match ty {
                $(
                    <$tty>::TYPE => {
                        let (trs, sk) = <$tty>::create(buf)?;
                        Ok((Box::new(trs), sk))
                    },
                )+
                _ => errf!("transaction type '{}' not find", ty)
            }
        }

        pub fn try_json_decode(ty: u8, json: &str) -> Ret<Option<Box<dyn Transaction>>> {
            match ty {
                $(
                    <$tty>::TYPE => {
                        let mut trs = <$tty>::default();
                        trs.from_json(json)?;
                        Ok(Some(Box::new(trs)))
                    },
                )+
                _ => Ok(None)
            }
        }

        pub fn transaction_json_decode(json: &str) -> Ret<Option<Box<dyn Transaction>>> {
            let obj = json_decode_object(json)?;
            let ty_str = if let Some(t) = obj.get("ty") {
                t
            } else {
                obj.get("type").ok_or_else(|| "transaction object JSON must have 'ty' or 'type'".to_string())?
            };
            let ty = ty_str.parse::<u8>().map_err(|_| format!("invalid transaction type: {}", ty_str))?;
            try_json_decode(ty, json)
        }

    };
}


// Trs list
combi_dynvec!{ DynVecTransaction, 
    Uint4, Transaction, transaction_create, transaction_json_decode
}





