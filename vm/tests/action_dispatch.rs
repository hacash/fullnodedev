#[cfg(test)]
mod action_dispatch {
    use field::{BytesW1, Field, Serialize};
    use vm::action::{self, EnvCoinbaseAddr, EnvHeight, TxMessage};

    #[test]
    fn try_create_dispatches_tx_message_roundtrip() {
        let mut src = TxMessage::new();
        src.data = BytesW1::from(b"dispatch-check".to_vec()).unwrap();
        let raw = src.serialize();

        let (act, sk) = action::try_create(TxMessage::KIND, &raw)
            .unwrap()
            .expect("TxMessage kind should be registered");
        assert_eq!(sk, raw.len());
        assert_eq!(act.kind(), TxMessage::KIND);

        let got = TxMessage::downcast(&act).expect("downcast to TxMessage");
        assert_eq!(got.data, src.data);
    }

    #[test]
    fn try_create_dispatches_env_coinbase_addr() {
        let src = EnvCoinbaseAddr::new();
        let raw = src.serialize();

        let (act, sk) = action::try_create(EnvCoinbaseAddr::KIND, &raw)
            .unwrap()
            .expect("EnvCoinbaseAddr kind should be registered");
        assert_eq!(sk, raw.len());
        assert_eq!(act.kind(), EnvCoinbaseAddr::KIND);
        assert!(EnvCoinbaseAddr::downcast(&act).is_some());
    }

    #[test]
    fn try_json_decode_dispatches_env_coinbase_addr() {
        let json = format!(r#"{{"kind":{}}}"#, EnvCoinbaseAddr::KIND);
        let act = action::try_json_decode(EnvCoinbaseAddr::KIND, &json)
            .unwrap()
            .expect("EnvCoinbaseAddr JSON decode should dispatch");
        assert_eq!(act.kind(), EnvCoinbaseAddr::KIND);
    }

    #[test]
    fn try_create_unknown_kind_returns_none() {
        let got = action::try_create(u16::MAX, &[]).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn try_json_decode_unknown_kind_returns_none() {
        let got = action::try_json_decode(u16::MAX, r#"{"kind":65535}"#).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn try_json_decode_kind_mismatch_returns_error() {
        let json = format!(r#"{{"kind":{}}}"#, EnvHeight::KIND);
        let err = action::try_json_decode(EnvCoinbaseAddr::KIND, &json).unwrap_err();
        assert!(err.contains("kind mismatch"), "{err}");
    }
}
