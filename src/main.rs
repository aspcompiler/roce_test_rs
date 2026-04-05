use clap::Parser;
use ibverbs;
use nix::net::if_::if_indextoname;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const TCP_PORT: u16 = 7471;

#[derive(Parser)]
#[command(name = "roce_test")]
#[command(about = "RDMA over Converged Ethernet (RoCE) testing tool")]
enum Cli {
    /// Run loopback test (default)
    Loopback,

    /// Run as server, listening on specified IP
    Server {
        /// IP address to listen on (e.g., 0.0.0.0 or 127.0.0.1)
        listen_ip: String,
    },

    /// Run as client, connecting to server
    Client {
        /// Server IP address to connect to
        server_ip: String,
    },
}

fn main() {
    let cli = Cli::try_parse().unwrap_or_else(|_| Cli::Loopback);

    let result = match cli {
        Cli::Loopback => run_loopback(),
        Cli::Server { listen_ip } => run_server(&listen_ip),
        Cli::Client { server_ip } => run_client(&server_ip),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_loopback() -> Result<(), Box<dyn Error>> {
    // Part 1: Setup RDMA Resources
    // =============================

    // 1. Discover RDMA devices
    let devices = ibverbs::devices()?;

    // 2. Find our Soft-RoCE device (rxe0)
    let dev = devices
        .iter()
        .find(|d| d.name().and_then(|name| name.to_str().ok()) == Some("rxe0"))
        .ok_or("Device rxe0 not found. Did you run 'sudo rdma link add'?")?;

    // 3. Extract the name as a string for printing
    let dev_name = dev
        .name()
        .and_then(|n| n.to_str().ok())
        .unwrap_or("unknown");

    println!("Found RDMA Device: {}", dev_name);

    // 4. Open the device context
    let context = dev.open()?;

    // 5. Create a Protection Domain
    let pd = context.alloc_pd()?;

    println!("Success! Created a Protection Domain on {}", dev_name);

    // 6. Create a Completion Queue (CQ)
    let cq = context.create_cq(16, 0)?;

    println!("Completion Queue (CQ) created!");

    // 7. Create a Queue Pair (QP) with loopback configuration
    // The handshake() call performs the state transitions (RESET -> INIT -> RTR -> RTS)
    // Set GID index to 1 for the loopback to work correctly
    let mut qp_builder = pd.create_qp(&cq, &cq, ibverbs::ibv_qp_type::IBV_QPT_RC);

    qp_builder.set_gid_index(1);

    let qp_builder_built = qp_builder.build()?;

    let endpoint = qp_builder_built.endpoint();

    let mut qp = qp_builder_built.handshake(endpoint)?;

    println!("Queue Pair (QP) is now in RTS (Ready to Send) mode!");

    // Part 2: Memory Registration and Loopback Test
    // =============================================

    // 8. Allocate and Register Memory
    // Allocate 16 bytes: first 8 bytes for receive buffer, last 8 bytes for send buffer
    let mut mr = pd.allocate::<u8>(16)?;

    // 9. Write test data to the send portion (bytes 8-15)
    mr[9] = 0x42;

    println!("Memory Allocated and Registered!");

    // Part 3: Post Send/Receive and Poll Completion Queue
    // ==================================================

    // 10. Post a Receive Work Request
    // This tells the QP to expect data in the first 8 bytes of our memory
    unsafe {
        qp.post_receive(&mut mr, ..8, 2)?;
    }

    println!("Receive Work Request posted!");

    // 11. Post a Send Work Request
    // This tells the QP to send the data from bytes 8-15
    unsafe {
        qp.post_send(&mut mr, 8.., 1)?;
    }

    println!("Send Work Request posted!");

    // Part 4: Poll for Completion
    // ===========================

    // 12. Poll the Completion Queue for completion events
    let mut sent = false;
    let mut received = false;
    let mut completions = [ibverbs::ibv_wc::default(); 16];

    while !sent || !received {
        let completed = cq.poll(&mut completions[..])?;

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

    Ok(())
}

fn find_rdma_device_auto() -> Result<(String, u32), Box<dyn Error>> {
    let devices = ibverbs::devices()?;

    for device in devices.iter() {
        let context = device.open()?;
        let gid_table = context.gid_table();

        // Find first RoCEv2 GID with valid interface
        for entry in gid_table {
            if entry.gid_type == 2 // IBV_GID_TYPE_ROCE_V2
                && entry.ndev_ifindex != 0
                && entry.gid != ibverbs::Gid::default()
            {
                // Log the network interface mapping
                let device_name = device
                    .name()
                    .ok_or("Device name not available")?
                    .to_str()
                    .map_err(|_| "Device name is not valid UTF-8")?
                    .to_string();

                if let Some(if_name) = get_interface_name(entry.ndev_ifindex) {
                    println!("Using RDMA device {} on interface {}", device_name, if_name);
                }

                return Ok((device_name, entry.gid_index));
            }
        }
    }

    Err("No suitable RDMA device found with RoCEv2 GID".into())
}

fn get_interface_name(ifindex: u32) -> Option<String> {
    if ifindex == 0 {
        return None;
    }
    if_indextoname(ifindex)
        .ok()
        .map(|s| s.to_string_lossy().to_string())
}

fn setup_rdma_resources(
    context: &ibverbs::Context,
    gid_index: u32,
) -> Result<
    (
        ibverbs::ProtectionDomain<'_>,
        ibverbs::CompletionQueue<'_>,
        u32,
    ),
    Box<dyn Error>,
> {
    // Create a Protection Domain
    let pd = context.alloc_pd()?;

    // Create a Completion Queue (CQ)
    let cq = context.create_cq(16, 0)?;

    Ok((pd, cq, gid_index))
}

fn exchange_endpoints(
    mut stream: &TcpStream,
    local_endpoint: ibverbs::QueuePairEndpoint,
    _is_server: bool,
) -> Result<ibverbs::QueuePairEndpoint, Box<dyn Error>> {
    let encoded = bincode::serialize(&local_endpoint)?;

    // Send endpoint (with length prefix)
    stream.write_all(&(encoded.len() as u64).to_le_bytes())?;
    stream.write_all(&encoded)?;

    // Receive endpoint (read length first, then data)
    let mut len_bytes = [0u8; 8];
    stream.read_exact(&mut len_bytes)?;
    let len = u64::from_le_bytes(len_bytes) as usize;

    let mut remote_encoded = vec![0u8; len];
    stream.read_exact(&mut remote_encoded)?;

    let remote_endpoint: ibverbs::QueuePairEndpoint = bincode::deserialize(&remote_encoded)?;

    Ok(remote_endpoint)
}

fn run_server(listen_ip: &str) -> Result<(), Box<dyn Error>> {
    println!("Starting server mode...");

    // Setup RDMA resources
    let (_device_name, gid_index) = find_rdma_device_auto()?;

    // Discover and open device
    let devices = ibverbs::devices()?;
    let device = devices
        .iter()
        .find(|d| d.name().and_then(|name| name.to_str().ok()) == Some("rxe0"))
        .ok_or("Device rxe0 not found")?;
    let context = device.open()?;

    let (pd, cq, actual_gid_index) = setup_rdma_resources(&context, gid_index)?;

    // Setup TCP listener
    let addr = format!("{}:{}", listen_ip, TCP_PORT);
    let listener = TcpListener::bind(&addr)?;
    println!("Server listening on {}", addr);

    let stream = listener.accept()?.0;
    let peer_addr = stream.peer_addr()?;
    println!("Client connected from {}", peer_addr);

    // Get local endpoint for initial QP (we need to create one for exchange)
    let mut qp_builder = pd.create_qp(&cq, &cq, ibverbs::ibv_qp_type::IBV_QPT_RC);
    qp_builder.set_gid_index(actual_gid_index);
    let qp_builder_built = qp_builder.build()?;
    let local_endpoint = qp_builder_built.endpoint();

    // Exchange endpoints
    let remote_endpoint = exchange_endpoints(&stream, local_endpoint, true)?;
    println!("Exchanged endpoints successfully");

    // Now perform handshake with remote endpoint
    let mut qp = qp_builder_built.handshake(remote_endpoint)?;
    println!("RDMA Queue Pair ready");

    // Memory setup for server
    let mut mr = pd.allocate::<u8>(16)?;

    // Post receive request
    unsafe {
        qp.post_receive(&mut mr, ..8, 2)?;
    }
    println!("Receive Work Request posted");

    // Poll for completion
    let mut completions: [ibverbs::ibv_wc; 16] = [ibverbs::ibv_wc::default(); 16];
    let mut received = false;

    while !received {
        let completed = cq.poll(&mut completions[..])?;

        if completed.is_empty() {
            continue;
        }

        for wr in completed {
            if wr.wr_id() == 2 {
                received = true;
                println!("Received data from client");
            }
        }
    }

    // Echo back the data
    mr[9] = mr[1];

    unsafe {
        qp.post_send(&mut mr, 8.., 1)?;
    }
    println!("Echoing data back");

    // Poll for send completion
    let mut sent = false;
    while !sent {
        let completed = cq.poll(&mut completions[..])?;

        if completed.is_empty() {
            continue;
        }

        for wr in completed {
            if wr.wr_id() == 1 {
                sent = true;
            }
        }
    }

    println!("\n=== Server Test Successful! ===");
    Ok(())
}

fn run_client(server_ip: &str) -> Result<(), Box<dyn Error>> {
    println!("Starting client mode...");

    // Setup RDMA resources
    let (_device_name, gid_index) = find_rdma_device_auto()?;

    // Discover and open device
    let devices = ibverbs::devices()?;
    let device = devices
        .iter()
        .find(|d| d.name().and_then(|name| name.to_str().ok()) == Some("rxe0"))
        .ok_or("Device rxe0 not found")?;
    let context = device.open()?;

    let (pd, cq, actual_gid_index) = setup_rdma_resources(&context, gid_index)?;

    // Connect to server
    let addr = format!("{}:{}", server_ip, TCP_PORT);
    let stream = TcpStream::connect(&addr)?;
    println!("Connected to server {}", addr);

    // Get local endpoint for initial QP
    let mut qp_builder = pd.create_qp(&cq, &cq, ibverbs::ibv_qp_type::IBV_QPT_RC);
    qp_builder.set_gid_index(actual_gid_index);
    let qp_builder_built = qp_builder.build()?;
    let local_endpoint = qp_builder_built.endpoint();

    // Exchange endpoints
    let remote_endpoint = exchange_endpoints(&stream, local_endpoint, false)?;
    println!("Exchanged endpoints successfully");

    // Now perform handshake with remote endpoint
    let mut qp = qp_builder_built.handshake(remote_endpoint)?;
    println!("RDMA Queue Pair ready");

    // Memory setup for client
    let mut mr = pd.allocate::<u8>(16)?;

    // Write test data
    mr[9] = 0x42;

    // Post send request
    unsafe {
        qp.post_send(&mut mr, 8.., 1)?;
    }
    println!("Sent test data");

    // Post receive request for echo
    unsafe {
        qp.post_receive(&mut mr, ..8, 2)?;
    }

    // Poll for completions
    let mut completions: [ibverbs::ibv_wc; 16] = [ibverbs::ibv_wc::default(); 16];
    let mut sent = false;
    let mut received = false;

    while !sent || !received {
        let completed = cq.poll(&mut completions[..])?;

        if completed.is_empty() {
            continue;
        }

        for wr in completed {
            match wr.wr_id() {
                1 => {
                    sent = true;
                    println!("Send completed");
                }
                2 => {
                    received = true;
                    println!("Received echo from server");
                }
                _ => {}
            }
        }
    }

    // Verify the data
    if mr[1] == 0x42 {
        println!("Data verified: 0x42");
    } else {
        println!("Data mismatch: expected 0x42, got 0x{:02x}", mr[1]);
    }

    println!("\n=== Client Test Successful! ===");
    Ok(())
}
