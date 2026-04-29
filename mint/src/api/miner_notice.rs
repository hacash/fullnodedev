fn miner_notice(
    ctx: ApiExecCtx,
    req: ApiRequest,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ApiResponse> + Send + 'static>> {
    Box::pin(async move {
        let target_height = req.query_u64("height", 0);
        let mut wait = req.query_u64("wait", 45);
        set_in_range!(wait, 1, 300);

        {
            let mut n = ctx.miner_worker_notice_count.lock().unwrap();
            *n += 1;
        }

        struct NoticeCountGuard {
            count: Arc<Mutex<u64>>,
        }

        impl Drop for NoticeCountGuard {
            fn drop(&mut self) {
                *self.count.lock().unwrap() -= 1;
            }
        }

        let _guard = NoticeCountGuard {
            count: ctx.miner_worker_notice_count.clone(),
        };

        let start_at = tokio::time::Instant::now();
        let wait_dur = Duration::from_secs(wait);
        let poll_dur = Duration::from_millis(250);

        loop {
            let current_height = ctx.engine.latest_block().height().uint();
            if target_height > 0 && current_height >= target_height {
                return api_ok(vec![("height", json!(current_height))]);
            }

            let elapsed = start_at.elapsed();
            if elapsed >= wait_dur {
                return api_ok(vec![("height", json!(current_height))]);
            }

            let left = wait_dur - elapsed;
            tokio::time::sleep(left.min(poll_dur)).await;
        }
    })
}
