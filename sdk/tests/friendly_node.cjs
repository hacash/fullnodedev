const create_hacash_sdk = require("../dist/js/hacashsdk.cjs");

async function run() {
    const sdk = await create_hacash_sdk();

    const acc1 = sdk.create_account("123456");
    const acc2 = sdk.create_account("654321");
    if (!acc1.address || !acc2.address) {
        throw new Error("create_account failed");
    }

    const tx = sdk.create_coin_transfer({
        main_prikey: "123456",
        to_address: acc2.address,
        fee: "1:244",
        hacash: "1:244",
        timestamp: 1755223764,
        satoshi: "0",
        chain_id: "0",
    });

    if (!tx.body || !tx.hash) {
        throw new Error("create_coin_transfer failed");
    }

    const sign_param = sdk.create_sign_tx_param({
        prikey: "123456",
        body: tx.body,
    });
    if (sign_param.prikey !== "123456" || sign_param.body !== tx.body) {
        throw new Error("create_sign_tx_param mapping failed");
    }

    console.log("friendly_node.cjs OK");
}

run().catch((err) => {
    console.error(err);
    process.exit(1);
});
