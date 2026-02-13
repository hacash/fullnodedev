(function(root) {
    const U64_MAX = (1n << 64n) - 1n;

    function pickField(input, names) {
        for (const name of names) {
            if (Object.prototype.hasOwnProperty.call(input, name) && input[name] !== undefined) {
                return input[name];
            }
        }
        return undefined;
    }

    function toU64BigInt(field, value) {
        let num = value;
        if (typeof num === "number") {
            if (!Number.isFinite(num) || !Number.isInteger(num)) {
                throw new TypeError(`${field} must be an integer`);
            }
            num = BigInt(num);
        } else if (typeof num === "string") {
            let s = num.trim();
            if (s.endsWith("n")) {
                s = s.slice(0, -1);
            }
            if (!/^\d+$/.test(s)) {
                throw new TypeError(`${field} must be a uint64 string`);
            }
            num = BigInt(s);
        } else if (typeof num !== "bigint") {
            throw new TypeError(`${field} must be number|string|bigint`);
        }
        if (num < 0n || num > U64_MAX) {
            throw new RangeError(`${field} out of uint64 range`);
        }
        return num;
    }

    function isInstanceOf(value, klass) {
        return typeof klass === "function" && value instanceof klass;
    }

    function ensureObjectParam(name, value) {
        if (value === undefined || value === null) {
            return {};
        }
        if (typeof value !== "object") {
            throw new TypeError(`${name} expects an object or wasm-bindgen class instance`);
        }
        return value;
    }

    function createFriendlyApi(rawApi) {
        if (typeof rawApi.create_account !== "function"
            || typeof rawApi.create_coin_transfer !== "function"
            || typeof rawApi.sign_transaction !== "function") {
            throw new Error("invalid hacash sdk backend exports");
        }

        const create_coin_transfer_param = (input) => {
            if (isInstanceOf(input, rawApi.CoinTransferParam)) {
                return input;
            }
            const src = ensureObjectParam("create_coin_transfer_param", input);
            const param = new rawApi.CoinTransferParam();

            const main_prikey = pickField(src, ["main_prikey"]);
            const from_prikey = pickField(src, ["from_prikey"]);
            const fee = pickField(src, ["fee"]);
            const to_address = pickField(src, ["to_address"]);
            const hacash = pickField(src, ["hacash"]);
            const diamonds = pickField(src, ["diamonds"]);
            const timestamp = pickField(src, ["timestamp"]);
            const satoshi = pickField(src, ["satoshi"]);
            const chain_id = pickField(src, ["chain_id"]);

            if (main_prikey !== undefined && main_prikey !== null) {
                param.main_prikey = String(main_prikey);
            }
            if (from_prikey !== undefined && from_prikey !== null) {
                param.from_prikey = String(from_prikey);
            }
            if (fee !== undefined && fee !== null) {
                param.fee = String(fee);
            }
            if (to_address !== undefined && to_address !== null) {
                param.to_address = String(to_address);
            }
            if (hacash !== undefined && hacash !== null) {
                param.hacash = String(hacash);
            }
            if (diamonds !== undefined && diamonds !== null) {
                param.diamonds = String(diamonds);
            }
            if (timestamp !== undefined && timestamp !== null) {
                param.timestamp = toU64BigInt("timestamp", timestamp);
            }
            if (satoshi !== undefined && satoshi !== null) {
                param.satoshi = toU64BigInt("satoshi", satoshi);
            }
            if (chain_id !== undefined && chain_id !== null) {
                param.chain_id = toU64BigInt("chain_id", chain_id);
            }

            return param;
        };

        const create_sign_tx_param = (input) => {
            if (isInstanceOf(input, rawApi.SignTxParam)) {
                return input;
            }
            const src = ensureObjectParam("create_sign_tx_param", input);
            const param = new rawApi.SignTxParam();
            const prikey = pickField(src, ["prikey"]);
            const body = pickField(src, ["body"]);
            if (prikey !== undefined && prikey !== null) {
                param.prikey = String(prikey);
            }
            if (body !== undefined && body !== null) {
                param.body = String(body);
            }
            return param;
        };

        const create_coin_transfer = (input) => rawApi.create_coin_transfer(create_coin_transfer_param(input));
        const sign_transaction = (input) => rawApi.sign_transaction(create_sign_tx_param(input));

        return {
            env: "browser-global",
            raw: rawApi,

            account_class: rawApi.Account,
            coin_transfer_param_class: rawApi.CoinTransferParam,
            coin_transfer_result_class: rawApi.CoinTransferResult,
            sign_tx_param_class: rawApi.SignTxParam,
            sign_tx_result_class: rawApi.SignTxResult,
            verify_address_result_class: rawApi.VerifyAddressResult,

            to_u64_bigint: (field, value) => toU64BigInt(field, value),
            create_coin_transfer_param,
            create_sign_tx_param,

            create_account: rawApi.create_account,
            hac_to_unit: rawApi.hac_to_unit,
            hac_to_mei: rawApi.hac_to_mei,
            verify_address: rawApi.verify_address,
            create_coin_transfer,
            sign_transaction,
        };
    }

    async function create_hacash_sdk() {
        if (typeof root.hacash_sdk !== "function") {
            throw new Error("window.hacash_sdk not found, load ./page/hacashsdk_bg.js first");
        }
        const rawApi = await root.hacash_sdk();
        return createFriendlyApi(rawApi);
    }

    root.hacash_sdk_api = root.hacash_sdk_api || {};
    root.hacash_sdk_api.create_hacash_sdk = create_hacash_sdk;
    root.create_hacash_sdk = create_hacash_sdk;
})(typeof globalThis !== "undefined" ? globalThis : window);
