### Prepare build environment (Ubuntu)

```sh
# dependencies
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
# optional tools for extra optimization / inspection
sudo apt install wabt binaryen
```

### Build all sdk artifacts

```sh
./pack.sh
```

Output directories:
- `dist/nodejs/*` raw wasm-bindgen (CommonJS)
- `dist/web/*` raw wasm-bindgen (ESM + async init)
- `dist/page/*` browser no-module bundle
- `dist/js/*` JS-friendly compatibility layer

### Three wasm result scenarios

1. Node.js load
- files:
  - `dist/nodejs/hacashsdk.js`
  - `dist/nodejs/hacashsdk_bg.wasm`
- recommended entry:
  - `dist/js/hacashsdk.cjs`

2. Browser JS with server hosting (HTTP fetch wasm)
- files:
  - `dist/web/hacashsdk.js`
  - `dist/web/hacashsdk_bg.wasm`
- recommended entry:
  - `dist/js/hacashsdk.mjs`

3. Pure static HTML page (no wasm file fetch, base64 inline decode)
- file:
  - `dist/page/hacashsdk_bg.js`
- example page:
  - `dist/page/friendly_test.html`
- required by some file:// static environments where direct wasm fetch is blocked.

### JS-friendly compatibility layer

`dist/js` contains:
- `hacashsdk.mjs`: one async entry for both Node.js and browser module environments
- `hacashsdk.cjs`: Node.js CommonJS bridge
- `hacashsdk.global.js`: browser no-module wrapper (works with `dist/page/hacashsdk_bg.js`)

Friendly layer features:
- auto environment detection (`node` / `web`)
- unified `snake_case` API only
- plain object param support for tx APIs
- automatic `u64` conversion (`number|string|bigint` -> `BigInt`) for:
  - `timestamp`
  - `satoshi`
  - `chain_id`

### Node.js usage (CommonJS)

```js
const create_hacash_sdk = require("./dist/js/hacashsdk.cjs");

(async () => {
  const sdk = await create_hacash_sdk();
  const acc = sdk.create_account("123456");
  console.log(acc.address);
})();
```

### Browser usage (ES module)

```js
import create_hacash_sdk from "./dist/js/hacashsdk.mjs";

const sdk = await create_hacash_sdk();
const v = sdk.hac_to_mei("1:244");
console.log(v);
```

### Browser no-module usage

```html
<script src="./dist/page/hacashsdk_bg.js"></script>
<script src="./dist/js/hacashsdk.global.js"></script>
<script>
  create_hacash_sdk().then((sdk) => {
    console.log(sdk.create_account("123456").address);
  });
</script>
```

### Quick smoke tests

```sh
node ./tests/friendly_node.cjs
node ./tests/friendly_node_esm.mjs
```

`dist/page/friendly_test.html` can be opened in browser for a no-module smoke test.
