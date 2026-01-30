



#[cfg(test)]
mod token_t {

    // use super::*;
    // use super::{Syntax, Tokenizer};

    #[test]
    fn t1(){
        /*
            use AnySwap = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            lib ERC20   = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS(1)
            let foo     = $0


            foo = 12 as u64

            var hei = env.block_height()
            var bar = hei + 100
            if hei < 3000 {
                ERC20::transfer(hei, 50)
            }else if hei < 6000 {
                bar = ERC20:get_status()
                return false
            }else{
                self.do_trs(200u64)
            }


            callcode ERC20::transfer

            return true



        */


        /* 


        let sss = r##"

            
            use AnySwap = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
            lib ERC20   = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS(1)

            callcode ERC20::do_transfer

            let abc = $0
            let num = $7

            abc = 8 as u32
            num = 2 as u64
            num = block_height()

            AnySwap.do_swap(abc, 50)

            ERC20:do_func1(abc)
            ERC20::do_static_func(abc)

            self.do_some_trs(num + 10)


            abc = sha3(0xABC123)
            abc = sha3("\"hacash\" \\\nworld")
            abc = sha3(0x0000111100001111)
            num = ripemd160(abc)

            num = check_signature(emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS)

            memory_put(abc, 24)
            num = memory_get(abc)

            if num > 100 {
                throw "panic error!"
            }

            while abc > 0 {
                abc = abc - 1
                num += 2
                if abc < 3 {
                    AnySwap.do_swap(abc, 50)
                    return memory_get(abc + 1)
                }
            }
            if num < 10 {
                assert abc >= 1
                num += 1
            } else if num < 5 {
                abc *= 2
            }else{
                return num + check_signature(num, abc*5, 2)
            }
            return (abc && 1) / (num - 1)
        "##;

        let tkmng1 = Tokenizer::new(sss.as_bytes());
        let tkmng2 = Tokenizer::new(sss.as_bytes());
        println!("tokens: {:?}", tkmng1.parse().unwrap());

        let sytax1 = Syntax::new(tkmng2.parse().unwrap());
        let (astblock, _) = sytax1.parse().unwrap();
        let irnodes = astblock.serialize().split_off(3);
        println!("asts len: {}", astblock.len());

        println!("irnodes: \n\n{}  len: {}\n\n", irnodes.to_hex(), irnodes.len());
        let opt_plain = crate::PrintOption::new("  ", 0);
        println!(
            "irnodes: \n\n{}\n\n{}\n\n",
            Formater::new(&opt_plain).print(&astblock),
            ircode_to_lang(&irnodes).unwrap()
        );
        let opt_desc = crate::PrintOption::new("    ", 0);
        println!(
            "irnodes: \n\n{}\n\n{}\n\n{}\n",
            sss,
            Formater::new(&opt_desc).print(&astblock),
            ircode_to_lang(&irnodes).unwrap()
        );
        let codes = astblock.codegen().unwrap();
        println!("bytecode: \n\n{}  len: {}\n\n", codes.to_hex(), codes.len());
        println!("bytecode: \n\n{}\n\n", codes.bytecode_print(false).unwrap());
        println!("bytecode: \n\n{}\n\n", codes.bytecode_print(true).unwrap());


        */


    }

}
