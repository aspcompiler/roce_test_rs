# Client-Server RDMA Implementation Summary

## What Was Added

### 1. Command-Line Interface (clap)
- **Loopback** (default): `roce_test`
- **Server mode**: `roce_test server <LISTEN_IP>`
- **Client mode**: `roce_test client <SERVER_IP>`

### 2. New Dependencies (Cargo.toml)
```toml
clap = "4.5"        # CLI argument parsing
bincode = "1.3"     # Binary serialization
serde = "1.0"       # Serialization framework
nix = "0.29"        # Network interface mapping
```

### 3. Core Functions Added

#### `find_rdma_device_auto()`
- Auto-discovers RDMA devices
- Queries GID table and finds first RoCEv2 GID
- Maps to network interface using `ndev_ifindex`
- Returns device, context, and GID index
- **Why**: Replaces hardcoded "rxe0" with automatic discovery

#### `get_interface_name(ifindex)`
- Helper to map kernel interface index to name (e.g., eth0)
- Uses `nix::net::if_::if_indextoname`
- **Why**: Provides user-friendly output showing which network interface is used

#### `setup_rdma_resources(device, context, gid_index)`
- Creates Protection Domain, Completion Queue, Queue Pair
- Sets GID index for remote capability
- Returns all four RDMA resource handles
- **Why**: Shared code between server and client modes

#### `exchange_endpoints(stream, local_endpoint, is_server)`
- Serializes local QueuePairEndpoint using bincode
- Sends length-prefixed binary data over TCP
- Receives remote endpoint same way
- Returns remote QueuePairEndpoint
- **Why**: TCP handshake to exchange RDMA connection metadata

#### `run_server(listen_ip)`
- Sets up RDMA resources using auto-discovery
- Listens on TCP port 7471
- Exchanges endpoints with client
- Posts receive request, waits for data
- Echoes data back to client
- **Why**: Server-side of two-machine test

#### `run_client(server_ip)`
- Sets up RDMA resources using auto-discovery
- Connects to server on TCP port 7471
- Exchanges endpoints with server
- Sends test data (0x42)
- Receives echo and verifies
- **Why**: Client-side of two-machine test

### 4. Key Design Decisions

**TCP Protocol**
- Simple binary format: `[8-byte length][serialized endpoint]`
- Uses bincode for efficient serialization
- No custom framing beyond length prefix

**GID Selection**
- Loopback: GID index 1 (unchanged)
- Remote: First RoCEv2 GID with valid network interface
- Rationale: RoCEv2 is standard for IP-based RDMA; queries avoid hardcoding

**Device Discovery**
- Iterates all devices and GID tables
- Checks `ndev_ifindex != 0` (ensures IP-capable device)
- Validates GID is non-zero and RoCEv2 type
- Logs which device/interface is selected

**Single Echo Pattern**
- Client sends 1 packet, server echoes 1 packet
- Simple verification without complexity of streams
- Easy to debug success/failure

### 5. File Changes

**Modified:**
- `Cargo.toml` - Added 4 new dependencies, fixed edition to 2021
- `src/main.rs` - Complete refactor into 7 functions + loopback preserved
- `README.md` - Added command examples and architecture explanation

**Preserved:**
- Loopback test logic unchanged (still works with `cargo run`)
- All original RDMA concepts and setup flow
- Comments explaining each step

## Testing Checklist

### Single Machine (Loopback)
```bash
cargo run
# Should show loopback test output (unchanged)
```

### Two Machine (Local Network)
**Terminal 1 (Server):**
```bash
cargo build --release
./target/release/roce_test server 0.0.0.0
```

**Terminal 2 (Client):**
```bash
./target/release/roce_test client <server_ip>
```

### Expected Flow
1. Both discover RDMA device and GID
2. Server listens on TCP port 7471
3. Client connects
4. Endpoints exchanged
5. RDMA Queue Pairs transition to RTS
6. Client sends 0x42
7. Server echoes back
8. Client verifies and succeeds

## Architecture Notes

### TCP Port
- Hardcoded to 7471 (configurable if needed)

### Memory Layout
- 16 bytes total: 8 recv + 8 send
- Send buffer at offset 8
- Receive buffer at offset 0
- Test data written to offset 9 (within send buffer)

### Work Request IDs
- ID 1: Send operations
- ID 2: Receive operations

### Error Handling
- Uses `Result<(), Box<dyn Error>>`
- Propagates with `?` operator
- Main catches errors and exits with code 1

## Future Enhancements

Possible extensions (not implemented):
- Configurable TCP port via CLI
- Multiple rounds of echo (stress test)
- Device/GID selection via CLI
- Larger buffer sizes for performance testing
- RDMA Write/Read operations (currently using Send/Receive only)
- Continuous server (don't exit after echo)
