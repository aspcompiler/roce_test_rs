use ibverbs;

fn main() {
    // 1. Discover RDMA devices
    let devices = ibverbs::devices().expect("Failed to get RDMA devices");
    
    // 2. Find our Soft-RoCE device (rxe0)
    // We convert the C name to a Rust string for the comparison
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
    let _pd = context.alloc_pd().expect("Failed to allocate Protection Domain");

    println!("Success! Created a Protection Domain on {}", dev_name);

    // 6. Allocate and Register Memory in one step
    // This allocates a buffer of 1024 integers (u8)
    // 'n' is the number of elements, not necessarily bytes.
    let mut mr = _pd.allocate::<u8>(1024)
        .expect("Failed to allocate and register memory");

    // 7. Accessing the data
    // High-level crates usually allow you to access the underlying slice
    mr[0] = 42; 

    println!("Memory Allocated and Registered!");
    // Usually, these crates hide lkey/rkey, but you can often find them:
    // println!("LKey: {}", mr.lkey());

    // 8. Create a Completion Queue (CQ)
    // min_cq_entries: 128 (how many "completion notes" it can hold)
    // id: 0 (our user-defined ID for this queue)
    let cq = context.create_cq(128, 0)
        .expect("Failed to create Completion Queue");

    println!("Completion Queue (CQ) created!");

    // 9. Create a Queue Pair (QP)
    // This links your Protection Domain and your CQ together.
    // We specify it as a 'Reliable Connection' (RC), which is the most common RoCE type.
    let qp_builder = _pd.create_qp(&cq, &cq, ibverbs::ibv_qp_type::IBV_QPT_RC);
    let _qp = qp_builder.build()
        .expect("Failed to create Queue Pair");

    println!("Queue Pair (QP) created and ready!");
}
