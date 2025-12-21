use dns_proxy::quic::create_quic_server_endpoint;

#[test]
fn test_quic_module_imports() {
    // Test that quic module exports are accessible
    // Verify the function exists (just check it compiles)
    let _ = create_quic_server_endpoint;
}
