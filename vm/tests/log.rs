#[cfg(test)]
#[allow(unused)]
mod hrc20 {

    use field::*;

    use vm::contract::*;
    use vm::ir::*;
    use vm::lang::*;
    use vm::rt::*;

    fn addr(s: &str) -> Address {
        Address::from_readable(s).unwrap()
    }

    #[test]
    fn test1() {
        // emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        Contract::new()
            // log
            .func(
                Func::new("render")
                    .unwrap()
                    .public()
                    .fitsh(
                        r##"
            param{ num }
            num = num + 1
            log("log1", num + 1)
            log("log2", 1, num + 2)
            log("log3", 1, 2, num + 3)
            log("log4", 1, 2, 3, num + 4)
            end
        "##,
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("12:244", 0);

        // main call
        Maincall::new()
            .fitsh(
                r##"
            lib C = 1
            C.render(123)
            end
        "##,
            )
            .unwrap()
            .testnet_call_print("8:244");
    }
}
