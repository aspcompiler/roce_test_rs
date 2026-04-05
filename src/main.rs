use ibverbs;

fn main() {
    // Part 1: Setup RDMA Resources
    // =============================

    // 1. Discover RDMA devices
    let devices = ibverbs::devices().expect("Failed to get RDMA devices");

    // 2. Find our Soft-RoCE device (rxe0)
    let dev = devices
        .iter()
        .find(|d| {
            d.name()
                .and_then(|name| name.to_str().ok()) == Some("rxe0")
        })
        .expect("Device rxe0 not found. Did you run 'sudo rdma link add'?");

    // 3. Extract the name as a string for printing
    let dev_name = dev.name()
        .and_then(|n| n.to_str().ok())
        .unwrap_or("unknown");

    println!("Found RDMA Device: {}", dev_name);

    // 4. Open the device context
    let context = dev.open().expect("Failed to open device context");

    // 5. Create a Protection Domain
    let pd = context.alloc_pd().expect("Failed to allocate Protection Domain");

    println!("Success! Created a Protection Domain on {}", dev_name);

    // 6. Create a Completion Queue (CQ)
    let cq = context.create_cq(16, 0)
        .expect("Failed to create Completion Queue");

    println!("Completion Queue (CQ) created!");

    // 7. Create a Queue Pair (QP) with loopback configuration
    // The handshake() call performs the state transitions (RESET -> INIT -> RTR -> RTS)
    // Set GID index to 1 for the loopback to work correctly
    let mut qp_builder = pd.create_qp(&cq, &cq, ibverbs::ibv_qp_type::IBV_QPT_RC);

    qp_builder.set_gid_index(1);

    let qp_builder_built = qp_builder.build()
        .expect("Failed to build Queue Pair");

    let endpoint = qp_builder_built.endpoint();

    let mut qp = qp_builder_built.handshake(endpoint)
        .expect("Failed to perform QP handshake (state transitions)");

    println!("Queue Pair (QP) is now in RTS (Ready to Send) mode!");

    // Part 2: Memory Registration and Loopback Test
    // =============================================

    // 8. Allocate and Register Memory
    // Allocate 16 bytes: first 8 bytes for receive buffer, last 8 bytes for send buffer
    let mut mr = pd.allocate::<u8>(16)
        .expect("Failed to allocate and register memory");

    // 9. Write test data to the send portion (bytes 8-15)
    mr[9] = 0x42;

    println!("Memory Allocated and Registered!");

    // Part 3: Post Send/Receive and Poll Completion Queue
    // ==================================================

    // 10. Post a Receive Work Request
    // This tells the QP to expect data in the first 8 bytes of our memory
    unsafe {
        qp.post_receive(&mut mr, ..8, 2)
            .expect("Failed to post receive request");
    }

    println!("Receive Work Request posted!");

    // 11. Post a Send Work Request
    // This tells the QP to send the data from bytes 8-15
    unsafe {
        qp.post_send(&mut mr, 8.., 1)
            .expect("Failed to post send request");
    }

    println!("Send Work Request posted!");

    // Part 4: Poll for Completion
    // ===========================

    // 12. Poll the Completion Queue for completion events
    let mut sent = false;
    let mut received = false;
    let mut completions = [ibverbs::ibv_wc::default(); 16];

    while !sent || !received {
        let completed = cq.poll(&mut completions[..])
            .expect("Failed to poll Completion Queue");

        if completed.is_empty() {
            continue;
        }

        assert!(completed.len() <= 2, "Unexpected number of completions");

        for wr in completed {
            match wr.wr_id() {
                1 => {
                    assert!(!sent, "Send completed twice!");
                    sent = true;
                    println!("Send completed successfully!");
                }
                2 => {
                    assert!(!received, "Receive completed twice!");
                    received = true;
                    // Verify the data was correctly looped back
                    assert_eq!(mr[1], 0x42, "Data mismatch in loopback!");
                    println!("Receive completed successfully!");
                }
                _ => panic!("Unexpected work request ID: {}", wr.wr_id()),
            }
        }
    }

    println!("\n=== Loopback Test Successful! ===");
    println!("Data was correctly sent and received locally!");
}
