fn miner_notice(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let target_height = req.query_u64("height", 0);
    let mut wait = req.query_u64("wait", 45);
    set_in_range!(wait, 1, 300);
    let _mwnc = MWNCount::new(ctx.miner_worker_notice_count.clone());
    
    let (lock, cvar) = &*BLOCK_NOTIFIER;
    let mut lasthei = lock.lock().unwrap();
    
    // Update to latest height first
    *lasthei = ctx.engine.latest_block().height().uint();

    if *lasthei >= target_height && target_height > 0 {
        return api_ok(vec![("height", json!(*lasthei))]);
    }

    let wait_dur = Duration::from_secs(wait);
    let start_at = std::time::Instant::now();
    loop {
        let current_height = ctx.engine.latest_block().height().uint();
        if current_height > *lasthei {
            *lasthei = current_height;
        }
        if target_height > 0 && *lasthei >= target_height {
            break;
        }

        let elapsed = start_at.elapsed();
        if elapsed >= wait_dur {
            break;
        }
        let left = wait_dur - elapsed;
        let result = cvar.wait_timeout(lasthei, left).unwrap();
        lasthei = result.0;
        if result.1.timed_out() {
            break;
        }
    }
    
    // Ensure we have the absolute latest
    let current_height = ctx.engine.latest_block().height().uint();
    if current_height > *lasthei {
        *lasthei = current_height;
    }

    api_ok(vec![("height", json!(*lasthei))])
}
