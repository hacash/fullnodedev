


### Run fullnode for dev 

```sh
cp ./hacash.config.ini ./target/debug/ && RUST_BACKTRACE=1 cargo run
```




#### start flow:

1. protocol::action::hook extend action
2. protocol::block::hook block hasher
4. create mem kv db / disk kv db
5. create mint checker
6. create block scaner
7. create chain engine
3. create memory tx pool
8. create p2p node
9. do start
10. 



#### interface:

1. block hasher
2. kv disk database
3. action adaptation
4. chain engine
5. minter
6. scaner

