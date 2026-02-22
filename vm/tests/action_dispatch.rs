#[cfg(test)]
mod action_dispatch {
    use field::{Field, Serialize};
    use vm::action::{self, EnvCoinbaseAddr, EnvHeight};

    #[test]
    fn try_create_dispatches_env_height_roundtrip() {
        let src = EnvHeight::new();
        let raw = src.serialize();

        let (act, sk) = action::try_create(EnvHeight::KIND, &raw)
            .unwrap()
            .expect("EnvHeight kind should be registered");
        assert_eq!(sk, raw.len());
        assert_eq!(act.kind(), EnvHeight::KIND);

        assert!(EnvHeight::downcast(&act).is_some());
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
