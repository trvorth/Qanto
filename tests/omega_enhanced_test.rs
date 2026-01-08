use qanto::omega_enhanced::{SandboxedExecutor, SecurityImpact};

#[test]
fn test_sandboxed_executor() {
    let executor = SandboxedExecutor::new(SecurityImpact::Medium);

    // Create a simple program:
    // 1. Write "key1" = "value1"
    // 2. Emit event "topic1" = "data1"

    let mut code = Vec::new();

    // Write "key1" = "value1"
    code.push(0x01); // WRITE_STATE
    code.push(4); // Key len
    code.extend_from_slice(b"key1");
    // Val len (u16 little endian) = 6
    code.push(6);
    code.push(0);
    code.extend_from_slice(b"value1");

    // Emit event "topic1" = "data1"
    code.push(0x03); // EMIT_EVENT
    code.push(6); // Topic len
    code.extend_from_slice(b"topic1");
    // Data len (u16 little endian) = 5
    code.push(5);
    code.push(0);
    code.extend_from_slice(b"data1");

    let result = executor.execute(&code).expect("Execution failed");

    assert_eq!(result.operations_count, 2);
    assert_eq!(result.state_changes.len(), 1);
    assert_eq!(result.state_changes.get("key1"), Some(&b"value1".to_vec()));
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].0, "topic1");
    assert_eq!(result.events[0].1, b"data1".to_vec());
}

#[test]
fn test_sandboxed_executor_limits() {
    // Test with Critical security impact (max 50 ops)
    let executor = SandboxedExecutor::new(SecurityImpact::Critical);

    let mut code = Vec::new();
    // Generate 51 ops
    for i in 0..51 {
        // DELETE_STATE "k{i}"
        code.push(0x02);
        let key = format!("k{}", i);
        code.push(key.len() as u8);
        code.extend_from_slice(key.as_bytes());
    }

    let result = executor.execute(&code);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Operation limit exceeded");
}
