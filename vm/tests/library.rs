mod common;

#[cfg(test)]
#[allow(unused)]
mod library {

    use field::*;

    use super::common::checked_compile_fitsh_to_ir;
    use sha2::digest::typenum::Abs;
    use vm::ContractAddress;
    use vm::action::*;
    use vm::contract::*;
    use vm::ir::*;
    use vm::lang::*;
    use vm::rt::*;

    fn addr(s: &str) -> Address {
        Address::from_readable(s).unwrap()
    }

    #[test]
    fn t1() {
        let ircode = checked_compile_fitsh_to_ir(
            "
            lib C = 0: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            var n1 = C:f1()
            end
            // return n1 + 2
        ",
        );
        println!("{}", ircode_to_lang(&ircode).unwrap());
    }

    #[test]
    fn deploy() {
        // emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        Contract::new()
            .func(Func::new("f1").public().fitsh("return 1").unwrap())
            .func(Func::new("f2").fitsh("return 2").unwrap())
            .testnet_deploy_print("8:244");

        // iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh
        Contract::new()
            .lib(addr("emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS"))
            .func(
                Func::new("f3")
                    .public()
                    .fitsh(
                        "
            lib C = 0: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            var n1 = C::f1()
            var n2 = C::f2()
            return n1 + n2
        ",
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("8:244", 1);

        // WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm
        Contract::new()
            .func(Func::new("f4").public().fitsh("return 4").unwrap())
            .testnet_deploy_print_by_nonce("8:244", 2);

        // bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5
        Contract::new()
            .func(Func::new("f5").public().fitsh("return 5").unwrap())
            .testnet_deploy_print_by_nonce("8:244", 3);

        // ocgMvMA9G9Gzmon5GDkugVbhY5DULpWVz
        Contract::new()
            .lib(addr("iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh"))
            .lib(addr("WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm"))
            .lib(addr("bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5"))
            .func(
                Func::new("f6")
                    .public()
                    .fitsh(
                        "
            lib C0 = 0
            lib C1 = 1
            lib C2 = 2
            print C0::f3()
            print C1::f4()
            print C2::f5()
            end
        ",
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("8:244", 4);
    }
}
