mod common;

#[cfg(test)]
#[allow(unused)]
mod call {

    use field::*;

    use super::common::{checked_compile_fitsh_to_ir, compile_fitsh_bytecode};
    use vm::contract::*;
    use vm::ir::*;
    use vm::lang::*;
    use vm::rt::*;

    fn addr(s: &str) -> Address {
        Address::from_readable(s).unwrap()
    }

    #[test]
    fn inh1() {
        // emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        Contract::new()
            .func(Func::new("f1").unwrap().public().fitsh("return 1").unwrap())
            .testnet_deploy_print_by_nonce("8:244", 0);

        // iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh
        Contract::new()
            .lib(addr("emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS"))
            .func(
                Func::new("f2")
                    .unwrap()
                    .fitsh(
                        "
            lib C = 0
            return C::f1() + 1
        ",
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("8:244", 1);

        // WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm
        Contract::new()
            .lib(addr("iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh"))
            .func(
                Func::new("f3")
                    .unwrap()
                    .fitsh(
                        "
            lib C = 0
            return C:f2() + 1
        ",
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("8:244", 2);

        // bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5
        Contract::new()
            .lib(addr("WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm"))
            .inh(addr("iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh"))
            .func(
                Func::new("f4")
                    .unwrap()
                    .public()
                    .fitsh(
                        "
            lib C = 0
            return C:f3() + self.f2() + 1
        ",
                    )
                    .unwrap(),
            )
            .testnet_deploy_print_by_nonce("8:244", 3);

        // main call
        Maincall::new()
            .fitsh(
                "
            lib C = 4
            print C.f4() + 1
            end
        ",
            )
            .unwrap()
            .testnet_call_print("8:244");
    }

    #[test]
    fn deploy() {
        let recursion_fn = r##"
            var num = pick(0)
            // print num 
            if num >= 30 {
                return "overflow"
            }
            var nk = "num_k"
            var mmm = memory_get(nk)
            if mmm is nil {
                mmm = 1
                memory_put(nk, mmm)
            }
            if mmm >= 10 {
                return "ok"
            }
            memory_put(nk, mmm + 1)
            return self.recursion(num + 1)
        "##;

        println!(
            "{}",
            compile_fitsh_bytecode(recursion_fn)
                .bytecode_print(false)
                .unwrap()
        );

        let contract = Contract::new().func(
            Func::new("recursion")
                .unwrap()
                .public()
                .fitsh(recursion_fn)
                .unwrap(),
        );
        // println!("\n\n{}\n\n", contract.serialize().to_hex());
        contract.testnet_deploy_print("8:244");
    }

    #[test]
    // fn call_recursion() {
    fn maincall1() {
        use vm::action::*;

        let maincodes = compile_fitsh_bytecode(
            r##"
            lib C = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            C.recursion(1)
            end
        "##,
        );

        println!("{}", maincodes.bytecode_print(true).unwrap());

        let act = ContractMainCall::from_bytecode(maincodes).unwrap();

        // print
        curl_trs_3(vec![Box::new(act)], "24:244");
    }
}
