/* Bytecode define Add one bytecode */

#[repr(u8)]
#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum Bytecode {
    #[default]
    ACTION = 0x00, // *@  call action
    ________________1 = 0x01,
    ________________2 = 0x02,
    ________________3 = 0x03,
    ________________4 = 0x04,
    ________________5 = 0x05,
    ACTVIEW = 0x06, // *@  call action view (read-only query)
    ACTENV = 0x07,  // *+  call action env
    NTENV = 0x08,   // *+  native env (VM state read)
    NTCTL = 0x09,   // *@  native runtime control (modify VM tx-local state)
    NTFUNC = 0x0a,  // *@  native pure function
    ________________11 = 0x0b,
    ________________12 = 0x0c,
    #[cfg(feature = "calcfunc")]
    CALCCALL = 0x0d, // ****@ local calc coprocessor call
    #[cfg(not(feature = "calcfunc"))]
    ________________13 = 0x0d,
    CODECALL = 0x0e,    // *,****
    CALL = 0x0f,        // **,****@
    CALLEXT = 0x10,     // *,****@
    CALLEXTVIEW = 0x11, // *,****@
    ________________18 = 0x12,
    ________________19 = 0x13,
    CALLUSEVIEW = 0x14, // *,****@
    CALLUSEPURE = 0x15, // *,****@
    ________________22 = 0x16,
    ________________23 = 0x17,
    CALLTHIS = 0x18,     //   ****@
    CALLSELF = 0x19,     //   ****@
    CALLSUPER = 0x1a,    //   ****@
    CALLSELFVIEW = 0x1b, //   ****@
    CALLSELFPURE = 0x1c, //   ****@
    ________________29 = 0x1d,
    ________________30 = 0x1e,
    ________________31 = 0x1f, // *@  native call
    PU8 = 0x20,                // *+     push u8
    PU16 = 0x21,               // **+    push u16
    PBUF = 0x22,               // *+     push buf
    PBUFL = 0x23,              // **+    push buf long
    P0 = 0x24,                 // +      push u8 0
    P1 = 0x25,                 // +      push u8 1
    P2 = 0x26,                 // +      push u8 2
    P3 = 0x27,                 // +      push u8 3
    PNIL = 0x28,               // +      push nil
    PNBUF = 0x29,              // +      push buf empty
    PTRUE = 0x2a,              // +      push true
    PFALSE = 0x2b,             // +      push false
    ________________44 = 0x2c,
    ________________45 = 0x2d,
    ________________46 = 0x2e,
    ________________47 = 0x2f,
    CU8 = 0x30,   // &      cast u8
    CU16 = 0x31,  // &      cast u16
    CU32 = 0x32,  // &      cast u32
    CU64 = 0x33,  // &      cast u64
    CU128 = 0x34, // &      cast u128
    ________________53 = 0x35,
    CBUF = 0x36,  // &      cast buf
    CTO = 0x37,   // *&     cast to
    TNIL = 0x38,  // &   is nil push Bool(true)
    TLIST = 0x39, // &   is compo list push Bool(true)
    TMAP = 0x3a,  // &   is compo map  push Bool(true)
    TIS = 0x3b,   // *&  is type id
    TID = 0x3c,   // &   type id
    ________________61 = 0x3d,
    ________________62 = 0x3e,
    ________________63 = 0x3f,
    DUP = 0x40,    // +      copy 0
    DUPN = 0x41,   // *+     copy u8
    POP = 0x42,    // a      pop top
    POPN = 0x43,   // *a...b pop n
    ROLL0 = 0x44,  // +      roll0
    ROLL = 0x45,   // *+     roll
    SWAP = 0x46,   // a,b++  swap  b,a = a,b
    REV = 0x47,    // a...b  reverse u8
    CAT = 0x48,    // a,b+   buf: b + a
    JOIN = 0x49,   // a...bn+
    BYTE = 0x4a,   // a,b+   val[n] = u8
    CUT = 0x4b,    // a,b,c+ cut buf (v, ost, len)+
    LEFT = 0x4c,   // *&     cut left  buf *
    RIGHT = 0x4d,  // *&     cut right buf *
    LDROP = 0x4e,  // *&     drop buf left *
    RDROP = 0x4f,  // *&     drop buf right *
    SIZE = 0x50,   // &      size (u16)
    CHOOSE = 0x51, // a,b,c+ (x ? a : b)
    ________________82 = 0x52,
    ________________83 = 0x53,
    ________________84 = 0x54,
    ________________85 = 0x55,
    ________________86 = 0x56,
    ________________87 = 0x57,
    ________________88 = 0x58,
    ________________89 = 0x59,
    ________________90 = 0x5a,
    ________________91 = 0x5b,
    ________________92 = 0x5c,
    ________________93 = 0x5d,
    ________________94 = 0x5e,
    ________________95 = 0x5f,
    NEWLIST = 0x60,    // + new compo list
    NEWMAP = 0x61,     // + new compo map
    PACKLIST = 0x62,   // (v...,n)+ pack compo list
    PACKMAP = 0x63,    // (v...,n)+ pack compo map
    INSERT = 0x64,     // t,k,v+  compo insert
    REMOVE = 0x65,     // t,k+    compo remove
    CLEAR = 0x66,      // t+      compo clear
    MERGE = 0x67,      // a,b+    compo merge
    LENGTH = 0x68,     // t+      compo length
    HASKEY = 0x69,     // t,k+    compo check has key
    ITEMGET = 0x6a,    // t,k+    compo iten get
    KEYS = 0x6b,       // &       compo keys
    VALUES = 0x6c,     // &       compo values
    HEAD = 0x6d,       // &       compo pick first
    BACK = 0x6e,       // &       compo pick last
    APPEND = 0x6f,     // &       compo append
    CLONE = 0x70,      // a++     compo clone
    UNPACK = 0x71,     // a       unpack sequence to local
    PACKTUPLE = 0x72,  // (v...,n)+ pack tuple value
    TUPLE2LIST = 0x73, // &       tuple to list
    _______________116 = 0x74,
    _______________117 = 0x75,
    _______________118 = 0x76,
    _______________119 = 0x77,
    _______________120 = 0x78,
    XLG = 0x79,      // *&    local logic
    XOP = 0x7a,      // *a    local operand
    GET = 0x7b,      // *+    local get
    PUT = 0x7c,      // *a,b  local put
    GETX = 0x7d,     // &     local x get
    PUTX = 0x7e,     // v,i   local x put
    ALLOC = 0x7f,    // *     local allocQ
    GET0 = 0x80,     // +     local get idx 0
    GET1 = 0x81,     // +     local get idx 1
    GET2 = 0x82,     // +     local get idx 2
    GET3 = 0x83,     // +     local get idx 3
    LOG1 = 0x84,
    LOG2 = 0x85,
    LOG3 = 0x86,
    LOG4 = 0x87,
    HSLICE = 0x88,   // a,b+  create heap slice
    HREADUL = 0x89,  // **+   heap read ul
    HREADU = 0x8a,   // *+    heap read u
    HWRITEXL = 0x8b, // **a   heap write xl
    HWRITEX = 0x8c,  // *a    heap write x
    HREAD = 0x8d,    // a,b+  heap read
    HWRITE = 0x8e,   // a,b   heap write
    HGROW = 0x8f,    // *     heap grow
    GGET = 0x90,     // &     global get
    GPUT = 0x91,     // a,b   global put
    MGET = 0x92,     // &     memory get
    MPUT = 0x93,     // a,b   memory put
    MTAKE = 0x94,    // &     memory take
    ________________152 = 0x98,
    SSTAT = 0x99, // &       storage info
    SLOAD = 0x9a, // &       storage load
    SEDIT = 0x9b, // a,b     storage edit
    SDEL = 0x9c,  // a       storage delete
    SNEW = 0x9d,  // a,b,c   storage create
    SRECV = 0x9e, // a,b     storage recover rent
    SRENT = 0x9f, // a,b   storage time rent
    AND = 0xa0,   // a,b+   and
    OR = 0xa1,    // a,b+   or
    EQ = 0xa2,    // a,b+   equal
    NEQ = 0xa3,   // a,b+   not equal
    LT = 0xa4,    // a,b+   less than
    GT = 0xa5,    // a,b+   great than
    LE = 0xa6,    // a,b+   less and eq
    GE = 0xa7,    // a,b+   great and eq
    NOT = 0xa8,   // a+   not
    ________________169 = 0xa9,
    ________________170 = 0xaa,
    BSHR = 0xab,                // a,b+   shr: >>
    BSHL = 0xac,                // a,b+   shl: <<
    BXOR = 0xad,                // a,b+   xor: ^
    BOR  = 0xae,                // a,b+   or:  |
    BAND = 0xaf,                // a,b+   and: &
    // arithmetic: binary core, div variants, pow, extrema, saturating, local inc/dec
    ADD         = 0xb0,         // a,b+   +
    SUB         = 0xb1,         // a,b+   -
    MUL         = 0xb2,         // a,b+   *
    DIV         = 0xb3,         // a,b+   /
    MOD         = 0xb4,         // a,b+   mod
    DIVUP       = 0xb5,         // a,b+   ceil(x/y)
    DIVROUND    = 0xb6,         // a,b+   round_half_up(x/y)
    POW         = 0xb7,         // a,b+   pow
    RPOW        = 0xb8,         // a,b,c+ fixed-point pow
    MAX         = 0xb9,         // a,b+   max
    MIN         = 0xba,         // a,b+   min
    SATADD      = 0xbb,         // a,b+   saturating add
    SATSUB      = 0xbc,         // a,b+   saturating sub
    ABSDIFF     = 0xbd,         // a,b+   abs(x-y)
    INC         = 0xbe,         // *&     += u8
    DEC         = 0xbf,         // *&     -= u8
    SQRT        = 0xc0,         // a+     floor isqrt(a)
    SQRTUP      = 0xc1,         // a+     ceil sqrt (min y with y*y >= a)
    ________________211 = 0xc2, // reserved
    // arithmetic: triple-input mul pipeline and mod
    ADDMOD      = 0xc3,         // a,b,c+ (x+y)%z
    MULMOD      = 0xc4,         // a,b,c+ (x*y)%z
    MULADD      = 0xc5,         // a,b,c+ (x*y)+z
    MULSUB      = 0xc6,         // a,b,c+ (x*y)-z
    MULDIV      = 0xc7,         // a,b,c+ (x*y)/z
    MULDIVUP    = 0xc8,         // a,b,c+ ceil((x*y)/z)
    MULDIVROUND = 0xc9,         // a,b,c+ round_half_up((x*y)/z)
    MULSHR      = 0xca,         // a,b,c+ (x*y)>>z
    MULSHRUP    = 0xcb,         // a,b,c+ ceil((x*y)/2^z)
    ________________212 = 0xcc, // reserved
    ________________213 = 0xcd, // reserved
    CLAMP       = 0xce,         // a,b,c+ clamp(x, lo, hi)
    DEVSCALED   = 0xcf,         // a,b,c+ abs(x-b)*c/b
    ________________214 = 0xd0, // reserved
    // arithmetic: four-input composites
    MULADDDIV   = 0xd1,         // a,b,c,d+ ((x*y)+z)/d
    MULSUBDIV   = 0xd2,         // a,b,c,d+ ((x*y)-z)/d
    MUL3DIV     = 0xd3,         // a,b,c,d+ (a*b*c)/d
    WITHINBPS   = 0xd4,         // a,b,c,d+ abs(a-b)*d <= b*c
    WAVG2       = 0xd5,         // a,b,c,d+ (a*b+c*d)/(b+d)
    LERP        = 0xd6,         // a,b,c,d+ linear interpolation
    ________________215 = 0xd7,
    ________________216 = 0xd8,
    ________________217 = 0xd9,
    ________________218 = 0xda,
    ________________219 = 0xdb,
    ________________220 = 0xdc,
    ________________221 = 0xdd,
    ________________222 = 0xde,
    ________________223 = 0xdf,
    JMPL = 0xe0,  // **    jump long
    JMPS = 0xe1,  // *     jump offset
    JMPSL = 0xe2, // **    jump offset long
    BRL = 0xe3,   // **a   branch long
    BRS = 0xe4,   // *a    branch offset
    BRSL = 0xe5,  // **a   branch offset long not_zero
    BRSLN = 0xe6, // **a   branch offset long is_zero
    ________________231 = 0xe7,
    ________________232 = 0xe8,
    ________________233 = 0xe9,
    PRT = 0xea,        // s     print for debug
    AST = 0xeb,        // c     assert throw
    ERR = 0xec,        // a     throw (ERR)
    ABT = 0xed,        //       abord
    RET = 0xee,        // a     func return (DATA)
    END = 0xef,        //       func return nil
    IRBYTECODE = 0xf0, // <IR NODE>
    IRLIST = 0xf1,     // <IR NODE>
    IRBLOCK = 0xf2,    // <IR NODE>
    IRBLOCKR = 0xf3,   // <IR NODE>
    IRIF = 0xf4,       // <IR NODE>
    IRIFR = 0xf5,      // <IR NODE>
    IRWHILE = 0xf6,    // <IR NODE>
    IRBREAK = 0xf7,    // <IR NODE>
    IRCONTINUE = 0xf8, // <IR NODE>
    ________________249 = 0xf9,
    ________________250 = 0xfa,
    ________________251 = 0xfb,
    ________________252 = 0xfc,
    BURN = 0xfd, // **    burn gas
    NOP = 0xfe,  //       do nothing
    NT = 0xff,   //       panic: never touch
}

use Bytecode::*;

impl From<Bytecode> for u8 {
    fn from(val: Bytecode) -> u8 {
        val as u8
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct BytecodeMetadata {
    pub valid: bool,
    pub param: u8,
    pub input: u8,
    pub otput: u8,
    pub intro: &'static str,
}

macro_rules! bytecode_metadata_define {
    ( $( $inst:ident : $p:expr, $i:expr, $o:expr , $s:ident)+ ) => {

impl Bytecode {

    pub fn metadata(&self) -> BytecodeMetadata {
        #[cfg(feature = "calcfunc")]
        if matches!(self, CALCCALL) {
            return BytecodeMetadata {
                valid: true,
                param: 4,
                input: 1,
                otput: 1,
                intro: "calc_call",
            }
        }
        match self {
            $(
            $inst => BytecodeMetadata {valid: true, param: $p, input: $i, otput: $o, intro: stringify!($s)},
            )+
            _ => BytecodeMetadata::default(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        #[cfg(feature = "calcfunc")]
        if s == "CALCCALL" {
            return Some(CALCCALL)
        }
        match s {
            $(
            stringify!($inst) => Some($inst),
            )+
            _ => None
        }
    }

}

macro_rules! bytecode_intro_sig {
    $(
        ($s) => {
            ($crate::rt::Bytecode::$inst, ($p) as usize, ($i) as usize, ($o) as usize)
        };
    )+
}

    };
}

/* params, stack input, stack output */
bytecode_metadata_define! {
    ACTION     : 1, 1, 0,     action  // no stack output; otput=0 to avoid extra POP in IRBLOCK
    ACTVIEW    : 1, 1, 1,     actview
    ACTENV     : 1, 0, 1,     actenv
    NTENV      : 1, 0, 1,     native_env
    NTCTL      : 1, 1, 1,     native_ctl
    NTFUNC     : 1, 1, 1,     native_func

    CODECALL     : 1+4, 1, 0,   code_call
    CALL         :   6, 1, 1,   call
    CALLEXT      : 1+4, 1, 1,   callext
    CALLEXTVIEW  : 1+4, 1, 1,   callextview
    CALLUSEVIEW  : 1+4, 1, 1,   calluseview
    CALLUSEPURE  : 1+4, 1, 1,   callusepure
    CALLTHIS     :   4, 1, 1,   callthis
    CALLSELF     :   4, 1, 1,   callself
    CALLSUPER    :   4, 1, 1,   callsuper
    CALLSELFVIEW :   4, 1, 1,   callselfview
    CALLSELFPURE :   4, 1, 1,   callselfpure

    PU8        : 1, 0, 1,     push_u8
    PU16       : 2, 0, 1,     push_u16
    PBUF       : 1, 0, 1,     push_buf
    PBUFL      : 2, 0, 1,     push_buf_long
    P0         : 0, 0, 1,     push_0
    P1         : 0, 0, 1,     push_1
    P2         : 0, 0, 1,     push_2
    P3         : 0, 0, 1,     push_3
    PNBUF      : 0, 0, 1,     push_empty_buf
    PNIL       : 0, 0, 1,     push_nil
    PTRUE      : 0, 0, 1,     push_true
    PFALSE     : 0, 0, 1,     push_false

    CU8        : 0, 1, 1,     cast_u8
    CU16       : 0, 1, 1,     cast_u16
    CU32       : 0, 1, 1,     cast_u32
    CU64       : 0, 1, 1,     cast_u64
    CU128      : 0, 1, 1,     cast_u128
    CBUF       : 0, 1, 1,     cast_bytes
    CTO        : 1, 1, 1,     cast_to
    TNIL       : 0, 1, 1,     type_is_nil
    TLIST      : 0, 1, 1,     type_is_list
    TMAP       : 0, 1, 1,     type_is_map
    TIS        : 1, 1, 1,     type_is
    TID        : 0, 1, 1,     type_id

    DUP        : 0, 0, 1,     dump
    DUPN       : 1, 0, 1,     dump_n
    POP        : 0, 255, 0,   pop
    POPN       : 1, 255, 0,   pop_n
    ROLL0      : 0, 0, 1,     roll_0
    ROLL       : 1, 0, 1,     roll
    SWAP       : 0, 2, 2,     swap
    REV        : 1, 255, 255, reverse
    CAT        : 0, 2, 1,     concat
    JOIN       : 1, 255, 1,   join
    BYTE       : 0, 2, 1,     byte
    CUT        : 0, 3, 1,     buf_cut
    LEFT       : 1, 1, 1,     buf_left
    RIGHT      : 1, 1, 1,     buf_right
    LDROP      : 1, 1, 1,     buf_left_drop
    RDROP      : 1, 1, 1,     buf_right_drop
    SIZE       : 0, 1, 1,     size
    CHOOSE     : 0, 3, 1,     choose

    NEWLIST    : 0, 0, 1,     new_list
    NEWMAP     : 0, 0, 1,     new_map
    PACKLIST   : 0, 255, 1,   pack_list
    PACKMAP    : 0, 255, 1,   pack_map
    INSERT     : 0, 3, 1,     insert
    REMOVE     : 0, 2, 1,     remove
    CLEAR      : 0, 1, 1,     clear
    MERGE      : 0, 2, 1,     merge
    LENGTH     : 0, 1, 1,     length
    HASKEY     : 0, 2, 1,     has_key
    ITEMGET    : 0, 2, 1,     item_get
    KEYS       : 0, 1, 1,     keys
    VALUES     : 0, 1, 1,     values
    HEAD       : 0, 1, 1,     head
    BACK       : 0, 1, 1,     back
    APPEND     : 0, 2, 1,     append
    CLONE      : 0, 1, 1,     clone
    PACKTUPLE  : 0, 255, 1,   pack_tuple
    TUPLE2LIST : 0, 1, 1,     tuple_to_list
    UNPACK     : 0, 2, 0,     unpack

    XLG        : 1, 1, 1,     local_logic
    XOP        : 1, 1, 0,     local_operand
    GET        : 1, 0, 1,     local
    PUT        : 1, 1, 0,     local_put
    GETX       : 0, 1, 1,     local_x
    PUTX       : 0, 2, 0,     local_x_put
    ALLOC      : 1, 0, 0,     local_alloc
    GET0       : 0, 0, 1,     local_0
    GET1       : 0, 0, 1,     local_1
    GET2       : 0, 0, 1,     local_2
    GET3       : 0, 0, 1,     local_3

    LOG1       : 0, 1, 0,     log_1
    LOG2       : 0, 2, 0,     log_2
    LOG3       : 0, 3, 0,     log_3
    LOG4       : 0, 4, 0,     log_4

    HSLICE     : 0, 2, 1,     heap_slice
    HREADUL    : 2, 0, 1,     heap_read_uint_long
    HREADU     : 1, 0, 1,     heap_read_uint
    HWRITEXL   : 2, 1, 0,     heap_write_xl
    HWRITEX    : 1, 1, 0,     heap_write_x
    HREAD      : 0, 2, 1,     heap_read
    HWRITE     : 0, 2, 0,     heap_write
    HGROW      : 1, 0, 0,     heap_grow

    GPUT       : 0, 2, 0,     global_put
    GGET       : 0, 1, 1,     global_get
    MPUT       : 0, 2, 0,     memory_put
    MGET       : 0, 1, 1,     memory_get
    MTAKE      : 0, 1, 1,     memory_take

    SSTAT      : 0, 1, 1,     storage_stat
    SLOAD      : 0, 1, 1,     storage_load
    SEDIT      : 0, 2, 0,     storage_edit
    SDEL       : 0, 1, 0,     storage_del
    SNEW       : 0, 3, 0,     storage_new
    SRECV      : 0, 2, 0,     storage_recv
    SRENT      : 0, 2, 0,     storage_rent

    AND        : 0, 2, 1,     and
    OR         : 0, 2, 1,     or
    EQ         : 0, 2, 1,     equal
    NEQ        : 0, 2, 1,     not_equal
    LT         : 0, 2, 1,     less_than
    GT         : 0, 2, 1,     greater_than
    LE         : 0, 2, 1,     less_equal
    GE         : 0, 2, 1,     greater_equal
    NOT        : 0, 1, 1,     not

    BSHR       : 0, 2, 1,     bit_shr
    BSHL       : 0, 2, 1,     bit_shl
    BXOR       : 0, 2, 1,     bit_xor
    BOR        : 0, 2, 1,     bit_or
    BAND       : 0, 2, 1,     bit_and

    ADD         : 0, 2, 1,     add
    SUB         : 0, 2, 1,     sub
    MUL         : 0, 2, 1,     mul
    DIV         : 0, 2, 1,     div
    MOD         : 0, 2, 1,     mod
    DIVUP       : 0, 2, 1,     div_up
    DIVROUND    : 0, 2, 1,     div_round
    POW         : 0, 2, 1,     pow
    RPOW        : 0, 3, 1,     rpow
    MAX         : 0, 2, 1,     max
    MIN         : 0, 2, 1,     min
    SATADD      : 0, 2, 1,     sat_add
    SATSUB      : 0, 2, 1,     sat_sub
    ABSDIFF     : 0, 2, 1,     abs_diff
    INC         : 1, 1, 1,     increase
    DEC         : 1, 1, 1,     decrease
    SQRT        : 0, 1, 1,     sqrt
    SQRTUP      : 0, 1, 1,     sqrt_up
    ADDMOD      : 0, 3, 1,     add_mod
    MULMOD      : 0, 3, 1,     mul_mod
    MULADD      : 0, 3, 1,     mul_add
    MULSUB      : 0, 3, 1,     mul_sub
    MULDIV      : 0, 3, 1,     mul_div
    MULDIVUP    : 0, 3, 1,     mul_div_up
    MULDIVROUND : 0, 3, 1,     mul_div_round
    MULSHR      : 0, 3, 1,     mul_shr
    MULSHRUP    : 0, 3, 1,     mul_shr_up
    CLAMP       : 0, 3, 1,     clamp
    DEVSCALED   : 0, 3, 1,     dev_scaled
    MULADDDIV   : 0, 4, 1,     mul_add_div
    MULSUBDIV   : 0, 4, 1,     mul_sub_div
    MUL3DIV     : 0, 4, 1,     mul3_div
    WITHINBPS   : 0, 4, 1,     within_bps
    WAVG2       : 0, 4, 1,     wavg2
    LERP        : 0, 4, 1,     lerp

    JMPL       : 2, 0, 0,     jump_long
    JMPS       : 1, 0, 0,     jump_offset
    JMPSL      : 2, 0, 0,     jump_offset_long
    BRL        : 2, 1, 0,     branch_long
    BRS        : 1, 1, 0,     branch_offset
    BRSL       : 2, 1, 0,     branch_offset_long
    BRSLN      : 2, 1, 0,     branch_offset_long_not

    RET        : 0, 1, 0,     return
    END        : 0, 0, 0,     end
    AST        : 0, 1, 0,     assert
    ERR        : 0, 1, 0,     throw
    ABT        : 0, 0, 0,     abort
    PRT        : 0, 1, 0,     print

    IRBYTECODE : 2, 255, 0,   ir_bytecode
    IRLIST     : 2, 255, 1,   ir_list
    IRBLOCK    : 2, 255, 0,   ir_block
    IRBLOCKR   : 2, 255, 1,   ir_block_expr
    IRIF       : 0, 3, 0,     ir_if
    IRIFR      : 0, 3, 1,     ir_if_expr
    IRWHILE    : 0, 2, 0,     ir_while
    IRBREAK    : 0, 0, 0,     ir_break
    IRCONTINUE : 0, 0, 0,     ir_continue

    BURN       : 2, 0, 0,     gas_burn
    NOP        : 0, 0, 0,     nop
    NT         : 0, 0, 0,     never_touch

}
