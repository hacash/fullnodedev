mod common;

#[cfg(test)]
#[allow(unused)]
mod main {

    use field::*;

    use super::common::{checked_compile_fitsh_to_ir, compile_fitsh_bytecode};
    use vm::action::*;
    use vm::contract::*;
    use vm::ir::*;
    use vm::lang::*;
    use vm::rt::*;

    #[test]
    fn deploy() {
        let _fn1 = r##"
            param { num }
            var data = memory_get("k")
            if data is nil {
                data = self.f2()
            }
            print data
            return num * 2 + 1
        "##;

        let _fn2 = r##"
            var gn = global_get("k")
            print gn
            return gn
        "##;

        // println!("{}", ircode_to_lang(&lang_to_ircode(&recursion_fn).unwrap()).unwrap());

        let contract = Contract::new()
            .func(Func::new("f1").public().fitsh(_fn1).unwrap())
            .func(Func::new("f2").fitsh(_fn2).unwrap());
        // println!("\n\n{}\n\n", contract.serialize().to_hex());
        contract.testnet_deploy_print("8:244");
    }

    #[test]
    fn call2() {
        let maincodes = compile_fitsh_bytecode(
            r##"
            lib C = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            global_put("k", 123 as u32)
            var n = C.f1(3)
            print n
            callcode C::f2
        "##,
        );

        let act = ContractMainCall::from_bytecode(maincodes).unwrap();
        // print
        curl_trs_3(vec![Box::new(act)], "24:244");
    }

    #[test]
    fn call1() {
        let maincodes = compile_fitsh_bytecode(
            r##"
            lib C = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            var num = C:f1(2)
            print num
            end
        "##,
        );

        println!("{}", maincodes.bytecode_print(true).unwrap());

        let act = ContractMainCall::from_bytecode(maincodes).unwrap();

        // print
        curl_trs_3(vec![Box::new(act)], "24:244");
    }
}
