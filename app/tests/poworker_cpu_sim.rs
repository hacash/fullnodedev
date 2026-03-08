use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use app::poworker::{PoWorkConf, poworker_with_stop};
use testkit::sim::miner_api::{MinerApiSim, MinerPendingStuff};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn poworker_cpu_mining_submit_success_with_sim_miner_api() {
    let _guard = test_guard();

    let sim = MinerApiSim::start(MinerPendingStuff::easy_for_test(1));
    let stop = Arc::new(AtomicBool::new(false));

    let cnf = PoWorkConf {
        rpcaddr: sim.rpcaddr().to_string(),
        supervene: 1,
        noncemax: 2048,
        noticewait: 1,
        useopencl: false,
        workgroups: 1,
        localsize: 256,
        unitsize: 64,
        opencldir: "x16rs/opencl/".to_string(),
        debug: 0,
        platformid: 0,
        deviceids: String::new(),
    };

    let stop2 = stop.clone();
    let worker = thread::spawn(move || {
        poworker_with_stop(cnf, Some(stop2));
    });

    let ok = sim.wait_for_submit(1, Duration::from_secs(8));
    stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(80));

    assert!(
        ok,
        "poworker did not submit mining success to simulated miner api"
    );

    let last_submit = sim.last_submit();
    assert_eq!(last_submit.get("height"), Some(&"1".to_string()));

    drop(sim);
    let _ = worker.join();
}
