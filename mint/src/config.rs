
#[derive(Clone, Copy)]
pub struct MintConf {
    pub chain_id: u64, // sub chain id
    pub sync_maxh: u64, // sync block of max height
    pub show_miner_name: bool,
    pub difficulty_group_blocks: u64, // reuses unstable_block value for PoW grouped sampling span
    pub difficulty_adjust_blocks: u64, // height : 288
    pub each_block_target_time: u64, // secs : 300
    pub test_coin: bool
    // pub _test_mul: u64,
}

#[derive(Clone, Copy)]
struct DifficultyWindowConf {
    adjust_blocks: u64,
    group_blocks: u64,
    target_time: u64,
}

impl DifficultyWindowConf {
    fn parse(sec: &HashMap<String, Option<String>>) -> DifficultyWindowConf {
        let adjust_blocks = ini_must_u64(sec, "difficulty_adjust_blocks", 288);
        let group_blocks = ini_must_u64(sec, "difficulty_group_blocks", ini_must_u64(sec, "unstable_block", 4));
        let target_time = ini_must_u64(sec, "each_block_target_time", 300);
        if group_blocks == 0 {
            panic!("config [mint].difficulty_group_blocks must be greater than 0")
        }
        if adjust_blocks == 0 {
            panic!("config [mint].difficulty_adjust_blocks must be greater than 0")
        }
        if adjust_blocks % group_blocks != 0 {
            panic!(
                "config [mint] difficulty window invalid: difficulty_adjust_blocks={} must be divisible by difficulty_group_blocks={}",
                adjust_blocks,
                group_blocks,
            )
        }
        DifficultyWindowConf { adjust_blocks, group_blocks, target_time }
    }
}

impl MintConf {

    pub fn is_mainnet(&self) -> bool {
        self.chain_id == 0
    }

    pub fn new(ini: &IniObj) -> MintConf {

        let sec = ini_section(ini, "mint");
        let diff = DifficultyWindowConf::parse(&sec);

        let cnf = MintConf {
            chain_id: ini_must_u64(&sec, "chain_id", 0),
            sync_maxh: ini_must_u64(&sec, "height_max", 0),
            show_miner_name: ini_must_bool(&sec, "show_miner_name", false),
            difficulty_adjust_blocks: diff.adjust_blocks, // 1 day
            difficulty_group_blocks: diff.group_blocks, // protocol reuses unstable_block numeric value for difficulty grouping
            each_block_target_time: diff.target_time, // 5 mins
            test_coin: ini_must_bool(&sec, "test_coin", false),
            // _test_mul: ini_must_u64(&sec, "_test_mul", 1), // test
        };

        cnf
    }


}

#[cfg(test)]
mod mint_config_tests {
    use super::*;

    #[test]
    #[should_panic(expected = "difficulty window invalid")]
    fn difficulty_window_requires_integral_grouping() {
        let mut ini = IniObj::new();
        let mut mint = HashMap::new();
        mint.insert("difficulty_adjust_blocks".to_string(), Some("288".to_string()));
        mint.insert("difficulty_group_blocks".to_string(), Some("7".to_string()));
        ini.insert("mint".to_string(), mint);
        let _ = MintConf::new(&ini);
    }
}
