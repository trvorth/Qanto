# Debug Session: wallet-live-e2e
- **Status**: [OPEN]
- **Issue**: `wallet_live_airdrop_send_history_e2e` 需要在真实本地测试网上完成创建钱包、资金注入、发送交易、历史确认与双边余额校验的全链路闭环验证。
- **Debug Server**: `http://127.0.0.1:7777/event`
- **Log File**: `.dbg/trae-debug-log-wallet-live-e2e.ndjson`

## Reproduction Steps
1. 保持本地三节点测试网运行，目标接口为 `127.0.0.1:50051` 与 `127.0.0.1:8081`。
2. 进入 `qanto-desktop-wallet/src-tauri`。
3. 运行 `cargo test wallet_live_airdrop_send_history_e2e -- --ignored --nocapture`。
4. 持续观察编译日志、测试日志与最终测试结果。

## Hypotheses & Verification
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | 测试会在原生依赖首次编译阶段长时间停留，但最终能自然完成并进入测试函数执行。 | High | Low | Confirmed in progress: 当前已实证进入 `librocksdb-sys` 的 `clang++`/`clang -cc1` 编译阶段。 |
| B | 钱包 keystore 与 `MockRuntime` 兼容改造仍有遗漏，导致测试在创建钱包、解锁或路径解析阶段失败。 | Med | Med | Pending |
| C | 资金注入路径与测试实现不一致，例如 `qanto_claimAirdrop` / `qanto_requestFaucetFunds` 的限流、地址规范化或金额假设不匹配，导致入账断言失败。 | High | Med | Pending |
| D | 交易广播或地址历史索引存在时序问题，导致发送成功后历史查询或余额确认窗口内未收敛。 | Med | Med | Pending |

## Log Evidence
- `.dbg/wallet-live-e2e.env` 已生成，当前会话已分配 `DEBUG_SERVER_URL=http://127.0.0.1:7777/event`。
- 运行证据显示当前 `cargo test wallet_live_airdrop_send_history_e2e -- --ignored --nocapture` 首先卡在 `Blocking waiting for file lock on build directory`。
- `ps` 证据确认有多条历史 `cargo test wallet_live_airdrop_send_history_e2e -- --ignored --nocapture` 进程仍在自然运行，其中 `librocksdb-sys` 的 `build-script-build` 与底层 `ar` 打包进程正在执行。
- 这表明当前阻塞来自前序原生依赖编译链尚未自然结束，而不是本轮测试已经进入业务断言。
- 前序锁链清空后重新发起最终测试，新的 `ps` 证据确认本轮已真实进入 `librocksdb-sys` 的 C++ 编译阶段，存在多个 `clang++` / `clang -cc1` 子进程在编译 RocksDB `transactions` 与 `write_batch_with_index` 相关源文件。
- `.dbg/wallet-live-e2e-final.log` 中可见多轮真实失败记录：先后出现 `RPC connect error (127.0.0.1:50051)`、`keystore dir create error: Operation not permitted (os error 1)`、`timed out waiting for confirmed balance` 以及 `Airdrop transfer failed: ... ΩMEGA ... unstable`。
- 当前运行中的三节点进程监听 `127.0.0.1:50051`、`127.0.0.1:8081`、`127.0.0.1:30303`、`127.0.0.1:30304`，且均来自 `target/release/qanto`。
- 时间戳证据显示 `target/release/qanto` 修改时间为 `2026-07-02 11:06:37`，而钱包发送路径修复文件 `qanto-desktop-wallet/src-tauri/src/tx.rs` 修改时间为 `2026-07-02 11:06:57`，说明当前运行环境尚未验证最新钱包 `UTXO` 修复。
- `Protocol_Core/qanto-node/src/qantodag.rs` 已包含 `info!` 级别的 `CREATE_CANDIDATE` 观测点，但当前 `validator1.log` 中尚未出现该日志；与此同时，旧错误 `Invalid block: Only the first transaction can be a coinbase` 仍大量存在，说明测试网很可能仍在基于旧行为验证历史广播交易或尚未完成针对最新代码的有效复测。
- 后续运行证据确认 `qanto_claimAirdrop` 可返回 `0x...` 交易哈希且 `airdrop_claims` 会登记，但在旧实现下 `/mempool` 始终为空、`/balance/{address}` 长时间保持 `0`，说明交易被“标记成功”却未真正落入本地 mempool。
- 代码对照证据确认根因：`AppState.p2p_command_sender` 与 `NodeRpcBackend` 均绑定到 `tx_to_p2p`，该通道只负责 `P2PServer::broadcast_message()`；而本地 mempool 接纳逻辑监听的是 `tx_from_p2p` / 直接 `mempool.add_transaction(...)` 路径，因此单笔 HTTP、gRPC 与 faucet/airdrop 提交存在“只广播、不落地”的断层。
- 修复后手工冒烟验证通过：对新地址执行 `qanto_claimAirdrop` 后，1 秒内 `/balance/{address}` 即稳定返回 `100.000000000`，证明空投交易已进入本地状态机并完成确认。
- 最终 `cargo test wallet_live_airdrop_send_history_e2e -- --ignored --nocapture` 输出 `test result: ok. 1 passed; 0 failed`，完整覆盖创建钱包、空投入账、发送交易、双边交易历史确认和余额更新。

## Verification Conclusion
- 假设 A 已确认：原生依赖编译耗时真实存在，但不是最终业务失败的唯一根因。
- 假设 C 部分确认：空投路径曾受 ΩMEGA 稳定性门控影响，属于环境侧可重现失败因子。
- 假设 D 部分确认：旧运行日志持续显示 `Only the first transaction can be a coinbase`，与钱包历史空输入转账路径完全一致。
- 新的关键结论：在重新构建并重启到最新代码前，当前运行中的测试网无法作为钱包 `UTXO` 修复是否生效的可信验证环境。
- 最终确认根因二：单笔交易和 faucet/airdrop 路径没有先把交易接纳到本地 mempool，导致 API/gRPC 返回成功但本地链状态与钱包查询路径完全不同步。
- 最终验证结论：钱包发送路径的 `UTXO` 修复与节点侧“本地接纳 + P2P 广播”修复同时生效后，Live E2E 已达到绿灯状态。
