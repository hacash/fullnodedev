
Call Entry:

    1. Main
    2. Abst

   
Contract Func Type:

    1. Abst
    2. User


Runtime Space:

    1. Oprand stack  (Func)
    2. Local stack   (Func)
    3. Heap          (Func)
    4. Memory        (Contract temp)
    5. Global        (Public temp)
    6. Storage       (Contract state)


Contract Deploy & Store:

    1. contract max size is 65535 byte = 64kb
    2. function max size is 65535 byte = 64kb
    3. deploy or update contract burn 90 fee


Contract Verify:

    1. irnode compile
    2. bytecode finish with END|RET inst
    3. bytecode inst valid
    4. bytecode param check
    5. bytecode jump dest
    6. CALLCODE must follow END op


Contract Code Store Fee:

    1. 50x tx fee


Contract KV State Rent:

    1. rent fee = period * (32 + datasize)
    2. one period = 300 block ( one day )
    3. period max = 10000 (about 30 years)
    4. ratain = 300 * 100 (about 100 days)
    5. data recover = rent it again before expire
    6. data len max = 1280 bytes = 32 * 40
    7. data type can store = Nil, Bool, Uint, Address, Bytes


Storage Entry Address:

    - Main => 1Mzbf...
    - P2sh => 3DrTG...
    - Abst => vFgHm...



Storage Write Ban:

    - Static Call


VM Logs:

    - Op:  Log1, Log2, Log3, Log4
    - Gas:   20,   24,   28,   32
    - 

Gas Calculation:

    - 1 gas = 1 byte
    - gas price = fee purity = txfeegot / txsize
    - 1 gcu = 32 gas or 32 byte
    - tx gas limit is 65535 / 4 = 16383
    - a machine execution charges at least 1 gcu of gas (32 gas) = gas / GSCU + 1
    - load a contract for call cost 2 * gcu = 64gas
    - call main cost gas at least 1 * gcu =  32gas
    - call abst cost gas at least 3 * gcu =  96gas
    - 


Call Kind:

    - Call        <libidx, fnsig>(argv)
    - ThisCall            <fnsig>(argv)
    - SelfCall            <fnsig>(argv)
    - SuperCall           <fnsig>(argv)
    - ViewCall    <libidx, fnsig>(argv)
    - PureCall    <libidx, fnsig>(argv)
    - CallCode     <libidx, fnsig>(argv)   // run callee code, inherit current ExecMode privileges, and forbid any nested call


Call Privileges:

    State is Global Value, Memory Value, Storage Data, Log Data.
    CallCode is NOT an ExecMode: it inherits the upper-level ExecMode privileges, and execution enters an "in_callcode" state where any CALL* instruction is forbidden.
    Ext actions are still gated by (mode, depth) rules.

    - Main          (State Write) => Outer,        View, Pure, Code
    - Abst          (State Write) =>        Inner, View, Pure, Code
    - P2sh          (State Write) =>               View, Pure, Code
    - View          (State Read ) =>               View, Pure, Code
    - Pure          (           ) =>                    Pure, Code
    - Code          (- inherit -) =>                               -
    - Outer | Inner (State Write) => Outer, Inner, View, Pure, Code (All types)


Call Context Change:

    - ctxadr (storage/log context) changes only on Outer
        - curadr (code owner for library resolution) follows resolved owner:
            Outer => callee, Inner => resolved child/parent, View/Pure => library; CallCode updates curadr to the resolved code owner


Inheritance Resolution (CALLTHIS/CALLSELF/CALLSUPER):

    - DFS search in inherits list order (current => parent => grandparent)
    - First match wins; inherits list order defines conflict priority
    - Cycle in inherits list is invalid and triggers InheritsError
    - Diamond inheritance is allowed; only true cycles are rejected


Abst Call Param:

    - Construct( bytes[?] )
    - Change( nil )
    - Append( nil )
    - PermitHAC(      to_addr[21], hacash[3~] )
    - PermitSAT(      to_addr[21], satoshi[8] )
    - PermitHACD(     to_addr[21], dianum[1], diamonds[6~] )
    - PermitAsset(    to_addr[21], serial[8], amount[8] )
    - PayableHAC(   from_addr[21], hacash[3~] )
    - PayableSAT(   from_addr[21], satoshi[8] )
    - PayableHACD(  from_addr[21], dianum[1], diamonds[6~] )
    - PayableAsset( from_addr[21], serial[8], amount[8] )


Add Opcode must Modified:

    1. Bytecode define enum     `./rt/bytecode.rs`
    2. Bytecode metadata table  `./rt/bytecode.rs`
    3. Gas table                `./rt/gas.rs`
    4. lang func define         `./rt/lang.rs`
    5. interpreter              `./interpreter/execute.rs`


Comparison Reference:

    1. Move VM
    2. Ethereum VM
    3. Solana VM
    4. Ton VM
    5. CKB VM
    6. EOS VM
    7. NEO VM
