

#[allow(dead_code)]
pub fn curl_trs_1(acts: Vec<Box<dyn Action>>) {
    curl_trs_2(acts, "")
}

pub fn curl_trs_2(acts: Vec<Box<dyn Action>>, fee: &str) {
    let acc = Account::create_by_password("123456").unwrap();
    let addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
    let fee = Amount::from(maybe!(fee.len()>0 ,fee, "8:244")).unwrap();
    let trs = TransactionType3::new_by(addr, fee, curtimes());
    curl_trs_fee(trs, acts, acc)
}

#[allow(dead_code)]
pub fn curl_trs_3(acts: Vec<Box<dyn Action>>, fee: &str) {
        /* emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5 ocgMvMA9G9Gzmon5GDkugVbhY5DULpWVz bJASBXHo5SbNWJWbfACqZVNmi2j2hhCpe ocgMvMA9G9Gzmon5GDkugVbhY5DULpWVz bJASBXHo5SbNWJWbfACqZVNmi2j2hhCpe Yk3wMvJAW1uHUYEbsCjEZrhPAfLWvL4LB kEAVFuGFjMkYPRVDqpLans1SDUYGyKqD5 ezXPRoMFaH2SQfY2MCrUFNp4t5nauQWQi fED9X4bJcGhzjjPrETTq1XjDj6RKTicqg */
    let acc = Account::create_by_password("123456").unwrap();
    let addrs = vec![
        Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap(),
        Address::from_readable("emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS").unwrap(),
        Address::from_readable("iW82ndGx4Qu9k3LE4iBaM9pUXUzGUmfPh").unwrap(),
        Address::from_readable("WF3hsfuqhA9a4n9Qx6Drrwv4p9P7yo5Dm").unwrap(),
        Address::from_readable("bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5").unwrap(),
        Address::from_readable("ocgMvMA9G9Gzmon5GDkugVbhY5DULpWVz").unwrap(),
        Address::from_readable("bJASBXHo5SbNWJWbfACqZVNmi2j2hhCpe").unwrap(),
        Address::from_readable("ocgMvMA9G9Gzmon5GDkugVbhY5DULpWVz").unwrap(),
        Address::from_readable("bJASBXHo5SbNWJWbfACqZVNmi2j2hhCpe").unwrap(),
        Address::from_readable("Yk3wMvJAW1uHUYEbsCjEZrhPAfLWvL4LB").unwrap(),
        Address::from_readable("kEAVFuGFjMkYPRVDqpLans1SDUYGyKqD5").unwrap(),
        Address::from_readable("ezXPRoMFaH2SQfY2MCrUFNp4t5nauQWQi").unwrap(),
        Address::from_readable("fED9X4bJcGhzjjPrETTq1XjDj6RKTicqg").unwrap(),
    ];
    let fee = Amount::from(maybe!(fee.len()>0 ,fee, "8:244")).unwrap();
    let mut trs = TransactionType3::new_by(addrs[0], fee, curtimes());
    trs.addrlist = AddrOrList::from_list(addrs).unwrap();
    curl_trs_fee(trs, acts, acc)
}


#[allow(dead_code)]
pub fn curl_trs_fee(mut trs: TransactionType3, acts: Vec<Box<dyn Action>>, acc: Account) {

    for act in acts {
        trs.push_action(act).unwrap();
    }

    trs.gas_max = Uint1::from(8);
    trs.fill_sign(&acc).unwrap();

    // println!("txsize:{}, feepay: {}, feegot: {}, feepurity: {}", trs.size(), trs.fee_pay(), trs.fee_got(), trs.fee_purity() );

    // print
    println!("\n");
    println!(r#"curl "http://127.0.0.1:8088/submit/transaction?hexbody=true" -X POST -d "{}""#, trs.serialize().to_hex());
    // println!("\n");
}

