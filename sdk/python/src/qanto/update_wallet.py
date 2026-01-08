import sys

try:
    # Read the original file
    with open('/Users/trevor/qanto/src/qantowallet.rs', 'r') as f:
        content = f.read()

    # Define markers
    start_marker = 'async fn subscribe_balance_ws('
    end_marker = '/// Subscribe to live balance updates via native P2P.'

    # Find positions
    start_idx = content.find(start_marker)
    end_idx = content.find(end_marker)

    if start_idx != -1 and end_idx != -1:
        # The new function content
        new_func = r'''async fn subscribe_balance_ws(
    api_address: &str,
    address: &str,
    finalized_only: bool,
) -> Result<()> {
    let url = format!("ws://{api_address}/ws");
    #[allow(deprecated)]
    let ws_cfg = WebSocketConfig {
        max_send_queue: None,
        max_write_buffer_size: 8 * 1024 * 1024,
        write_buffer_size: 64 * 1024,
        max_message_size: Some(512 * 1024),
        max_frame_size: Some(512 * 1024),
        accept_unmasked_frames: false,
    };
    let mut backoff_ms: u64 = 500; // start with 0.5s, exponential up to 30s
    let max_backoff_ms: u64 = 30_000;
    let cache = AccountStateCache::new();

    loop {
        let (ws_stream, _resp) = match connect_async_with_config(&url, Some(ws_cfg), false).await {
            Ok(tuple) => tuple,
            Err(e) => {
                eprintln!(
                    "⚠️ WebSocket connect failed ({e}). Retrying in {}ms...",
                    backoff_ms
                );
                let jitter: u64 = rand::thread_rng().gen_range(0..=250);
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + jitter)).await;
                backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
                continue;
            }
        };

        let (mut write, mut read) = ws_stream.split();

        // Build filters (server expects string-typed filters)
        let mut filters = serde_json::Map::new();
        filters.insert(
            "address".to_string(),
            serde_json::Value::String(address.to_string()),
        );
        if finalized_only {
            filters.insert(
                "finalized_only".to_string(),
                serde_json::Value::String("true".to_string()),
            );
        }

        let sub_msg = serde_json::json!({
            "type": "subscribe",
            "subscription_type": "balances",
            "filters": filters
        })
        .to_string();

        if let Err(e) = write.send(Message::Text(sub_msg)).await {
            eprintln!("⚠️ Failed to send subscribe message ({e}). Reconnecting...");
            // drop connection and retry outer loop
            let jitter: u64 = rand::thread_rng().gen_range(0..=250);
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + jitter)).await;
            backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
            continue;
        }

        // Proactively request a snapshot (harmless if server ignores)
        let snapshot_msg = serde_json::json!({
            "type": "balance_snapshot_request",
            "subscription_type": "balances",
            "filters": filters.clone()
        })
        .to_string();
        let _ = write.send(Message::Text(snapshot_msg)).await;

        if finalized_only {
            println!(
                "📡 Subscribed to FINALIZED-ONLY balance updates for {address} via WebSocket at {api_address}"
            );
        } else {
            println!(
                "📡 Subscribed to balance updates for {address} via WebSocket at {api_address}"
            );
        }

        // Reset backoff on successful subscribe
        backoff_ms = 500;

        // Stream messages until error/close, then auto-reconnect
        let mut current_total_confirmed: Option<u64> = None;
        let mut current_total_unconfirmed: Option<i64> = None;
        // Zero-balance fallback if no snapshot arrives quickly
        let initial_deadline = std::time::Instant::now() + Duration::from_millis(1200);
        let mut initial_printed_zero = false;
        let mut last_heartbeat = std::time::Instant::now();
        let mut chunk_buf: std::collections::HashMap<String, (usize, Vec<Option<String>>)> = std::collections::HashMap::new();
        
        println!("\n(Press Ctrl+C to quit dashboard)");

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\n🛑 Exiting Real-Time Dashboard...");
                    return Ok(());
                }
                maybe_msg = read.next() => {
                    let msg = match maybe_msg {
                        Some(m) => m,
                        None => {
                            eprintln!("⚠️ Stream ended by server.");
                            break; // break inner loop to reconnect
                        }
                    };

                    match msg {
                        Ok(Message::Text(text)) => {
                            let v: serde_json::Value = match serde_json::from_str(&text) {
                                Ok(json) => json,
                                Err(_) => continue,
                            };
                            let t = v.get("type").and_then(|t| t.as_str());
                            if t == Some("chunk_start") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                if !id.is_empty() && total > 0 {
                                    chunk_buf.insert(id, (total, vec![None; total]));
                                }
                                continue;
                            }
                            if t == Some("chunk_data") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let index = v.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                                let data = v.get("data").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                if let Some((total, slots)) = chunk_buf.get_mut(&id) {
                                    if index < *total {
                                        slots[index] = Some(data);
                                    }
                                }
                                continue;
                            }
                            if t == Some("chunk_end") {
                                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                if let Some((_total, slots)) = chunk_buf.remove(&id) {
                                    if slots.iter().all(|s| s.is_some()) {
                                        let assembled: String = slots.into_iter().map(|s| s.unwrap()).collect();
                                        if let Ok(av) = serde_json::from_str::<serde_json::Value>(&assembled) {
                                            if let Some(tt) = av.get("type").and_then(|t| t.as_str()) {
                                                match tt {
                                                    "balance_snapshot" => {
                                                        if let Some((addr, total_c, total_u, _ts)) = parse_ws_balance_snapshot(&av) {
                                                            cache.set_balance(&addr, total_c as u128);
                                                            let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                                            let confirmed = (total_c as f64) / base;
                                                            let pending = (total_u as f64) / base;
                                                            print!("\x1B[2J\x1B[1;1H");
                                                            println!("============================================================");
                                                            println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                                            println!("============================================================");
                                                            println!("📍 Address:  {addr}");
                                                            println!("🔗 Node:     {api_address}");
                                                            println!("------------------------------------------------------------");
                                                            println!(
                                                                "📊 Balance:  {:.6} QNT (Confirmed)", confirmed
                                                            );
                                                            println!(
                                                                "⏳ Pending:  {:.6} QNT (Mempool)", pending
                                                            );
                                                            println!("------------------------------------------------------------");
                                                            println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                                            current_total_confirmed = Some(total_c);
                                                            current_total_unconfirmed = Some(total_u as i64);
                                                        }
                                                    }
                                                    "balance_delta_update" => {
                                                        if let Some((addr, dc, du, _finalized, _ts)) = parse_ws_balance_delta(&av) {
                                                            let cur_c = current_total_confirmed.unwrap_or(0);
                                                            let cur_u = current_total_unconfirmed.unwrap_or(0);
                                                            let new_c = (cur_c as i128 + dc as i128).max(0) as u64;
                                                            let new_u = (cur_u as i128 + du as i128).max(0) as i64;
                                                            cache.set_balance(&addr, new_c as u128);
                                                            let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                                            print!("\x1B[2J\x1B[1;1H");
                                                            println!("============================================================");
                                                            println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                                            println!("============================================================");
                                                            println!("📍 Address:  {addr}");
                                                            println!("🔗 Node:     {api_address}");
                                                            println!("------------------------------------------------------------");
                                                            println!(
                                                                "📊 Balance:  {:.6} QNT (Confirmed)", (new_c as f64)/base
                                                            );
                                                            println!(
                                                                "⏳ Pending:  {:.6} QNT (Mempool)", (new_u as f64)/base
                                                            );
                                                            println!("------------------------------------------------------------");
                                                            println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                                            current_total_confirmed = Some(new_c);
                                                            current_total_unconfirmed = Some(new_u);
                                                            initial_printed_zero = true;
                                                        }
                                                    }
                                                    "balance_update" => {
                                                        if let Some((addr, wb, _finalized, _ts)) = parse_ws_balance_update(&av) {
                                                            cache.set_balance(&addr, wb.total_confirmed as u128);
                                                            let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                                            let confirmed = (wb.total_confirmed as f64) / base;
                                                            let pending = (wb.unconfirmed_delta as f64) / base;
                                                            print!("\x1B[2J\x1B[1;1H");
                                                            println!("============================================================");
                                                            println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                                            println!("============================================================");
                                                            println!("📍 Address:  {addr}");
                                                            println!("🔗 Node:     {api_address}");
                                                            println!("------------------------------------------------------------");
                                                            println!(
                                                                "📊 Balance:  {:.6} QNT (Confirmed)", confirmed
                                                            );
                                                            println!(
                                                                "⏳ Pending:  {:.6} QNT (Mempool)", pending
                                                            );
                                                            println!("------------------------------------------------------------");
                                                            println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                                            current_total_confirmed = Some(wb.total_confirmed);
                                                            current_total_unconfirmed = Some(wb.unconfirmed_delta as i64);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                                continue;
                            }
                            match t {
                                Some("balance_snapshot") => {
                                    if let Some((addr, total_c, total_u, _ts)) = parse_ws_balance_snapshot(&v) {
                                        cache.set_balance(&addr, total_c as u128);
                                        let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                        let confirmed = (total_c as f64) / base;
                                        let pending = (total_u as f64) / base;
                                        print!("\x1B[2J\x1B[1;1H");
                                        println!("============================================================");
                                        println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                        println!("============================================================");
                                        println!("📍 Address:  {addr}");
                                        println!("🔗 Node:     {api_address}");
                                        println!("------------------------------------------------------------");
                                        println!(
                                            "📊 Balance:  {:.6} QNT (Confirmed)", confirmed
                                        );
                                        println!(
                                            "⏳ Pending:  {:.6} QNT (Mempool)", pending
                                        );
                                        println!("------------------------------------------------------------");
                                        println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                        current_total_confirmed = Some(total_c);
                                        current_total_unconfirmed = Some(total_u as i64);
                                    }
                                }
                                Some("balance_delta_update") => {
                                    if let Some((addr, dc, du, _finalized, _ts)) = parse_ws_balance_delta(&v) {
                                        let cur_c = current_total_confirmed.unwrap_or(0);
                                        let cur_u = current_total_unconfirmed.unwrap_or(0);
                                        let new_c = (cur_c as i128 + dc as i128).max(0) as u64;
                                        let new_u = (cur_u as i128 + du as i128).max(0) as i64;
                                        cache.set_balance(&addr, new_c as u128);
                                        let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                        print!("\x1B[2J\x1B[1;1H");
                                        println!("============================================================");
                                        println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                        println!("============================================================");
                                        println!("📍 Address:  {addr}");
                                        println!("🔗 Node:     {api_address}");
                                        println!("------------------------------------------------------------");
                                        println!(
                                            "📊 Balance:  {:.6} QNT (Confirmed)", (new_c as f64)/base
                                        );
                                        println!(
                                            "⏳ Pending:  {:.6} QNT (Mempool)", (new_u as f64)/base
                                        );
                                        println!("------------------------------------------------------------");
                                        println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                        current_total_confirmed = Some(new_c);
                                        current_total_unconfirmed = Some(new_u);
                                        initial_printed_zero = true;
                                    }
                                }
                                Some("balance_update") => {
                                    if let Some((addr, wb, _finalized, _ts)) = parse_ws_balance_update(&v) {
                                        cache.set_balance(&addr, wb.total_confirmed as u128);
                                        let base = crate::transaction::SMALLEST_UNITS_PER_QAN as f64;
                                        let confirmed = (wb.total_confirmed as f64) / base;
                                        let pending = (wb.unconfirmed_delta as f64) / base;
                                        print!("\x1B[2J\x1B[1;1H");
                                        println!("============================================================");
                                        println!("             QANTO REAL-TIME WALLET DASHBOARD               ");
                                        println!("============================================================");
                                        println!("📍 Address:  {addr}");
                                        println!("🔗 Node:     {api_address}");
                                        println!("------------------------------------------------------------");
                                        println!(
                                            "📊 Balance:  {:.6} QNT (Confirmed)", confirmed
                                        );
                                        println!(
                                            "⏳ Pending:  {:.6} QNT (Mempool)", pending
                                        );
                                        println!("------------------------------------------------------------");
                                        println!("Last update: {}", chrono::Local::now().format("%H:%M:%S"));
                                        current_total_confirmed = Some(wb.total_confirmed);
                                        current_total_unconfirmed = Some(wb.unconfirmed_delta as i64);
                                    }
                                }
                                Some("subscription_confirmed") | Some("balance_subscription_confirmed") => {
                                    // Optional: acknowledge confirmation
                                    if let Some(cid) = v.get("client_id").and_then(|c| c.as_str()) {
                                        println!("✅ Subscription confirmed (client_id={cid})");
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok(Message::Ping(data)) => {
                            // Respond to heartbeat pings to keep connection alive
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Ok(Message::Close(frame)) => {
                            let reason = frame
                                .as_ref()
                                .map(|f| f.reason.to_string())
                                .unwrap_or_else(|| "server close".to_string());
                            println!("🔌 WebSocket closed ({reason}). Reconnecting...");
                            break; // break inner loop to reconnect
                        }
                        Err(e) => {
                            eprintln!("⚠️ WebSocket error: {e}. Reconnecting...");
                            break; // break inner loop to reconnect
                        }
                        _ => {}
                    }
                }
            }
            
            // Check timeout inside loop (simplified)
            if !initial_printed_zero && std::time::Instant::now() >= initial_deadline {
                 // Only print if we haven't cleared screen yet
                 println!("... retrieving balance ...");
                 initial_printed_zero = true;
            }
            
            if last_heartbeat.elapsed() >= Duration::from_secs(15) {
                 let _ = write.send(Message::Ping(Vec::new())).await;
                 last_heartbeat = std::time::Instant::now();
            }
        }

        // Backoff before reconnecting
        let jitter: u64 = rand::thread_rng().gen_range(0..=250);
        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms + jitter)).await;
        backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
    }
}
'''
        # Write updated content
        with open('/Users/trevor/qanto/src/qantowallet.rs', 'w') as f:
            f.write(content[:start_idx] + new_func + '\n' + content[end_idx:])
        print("Successfully updated qantowallet.rs")
    else:
        print(f'Markers not found: {start_idx}, {end_idx}', file=sys.stderr)
        sys.exit(1)

except Exception as e:
    print(e, file=sys.stderr)
    sys.exit(1)
