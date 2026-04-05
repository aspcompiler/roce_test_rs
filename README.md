# Testing ROCE using soft-roce

This instructions is create from running on Ubuntu 24.04 t3.medium.

## Install packages

```
sudo apt update

# Install extra kernel modules package stripped out by AWS that contains the RDMA drivers
sudo apt install linux-modules-extra-$(uname -r)

# Install RDMA
sudo apt install -y rdma-core ibverbs-utils rdmacm-utils perftest

# Install dev tools
sudo apt install -y build-essential cmake libibverbs-dev pkg-config
sudo apt install -y pkg-config python3-dev
sudo apt install -y clang libclang-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Load the Soft-RoCE Kernel Module

```
sudo modprobe rdma_rxe
```

## Bind Soft-RoCE to your Ethernet Interface

```
ip addr
sudo rdma link add rxe0 type rxe netdev eth0

# Check the RDMA link status:
rdma link

# Check the Verbs device
ibv_devinfo
```
## Run Cargo Program

### Loopback Mode (Default)
```bash
cargo run
```
Runs a self-contained loopback test on a single Queue Pair.

### Server Mode
```bash
cargo run -- server 0.0.0.0
```
Starts a server listening on the specified IP address (0.0.0.0 for all interfaces, 127.0.0.1 for localhost).

### Client Mode
```bash
cargo run -- client <SERVER_IP>
```
Connects to a server at the specified IP and runs an echo test.

### Two-Machine Testing Example
**Machine 1 (Server):**
```bash
cargo build --release
./target/release/roce_test server 0.0.0.0
```

**Machine 2 (Client):**
```bash
./target/release/roce_test client <SERVER_IP>
```
Replace `<SERVER_IP>` with the actual server IP address.

## RDMA Concepts

This section explains the key RDMA abstractions used in this project:

### Protection Domain (PD)
A Protection Domain is a security boundary that isolates RDMA resources. Think of it as a namespace that contains all the memory, queue pairs, and other objects associated with a particular application or context. All other RDMA resources (Memory Regions, Queue Pairs, Completion Queues) must belong to a Protection Domain. Resources in different PDs cannot directly access each other.

### Memory Region (MR)
A Memory Region is registered virtual memory that can be accessed via RDMA operations. When you allocate a buffer for RDMA, it must be registered with the RDMA device. Registration pins the memory in RAM, prevents the OS from swapping it out, and generates local and remote access keys (lkey and rkey) that allow the device and remote peers to access the memory. In this project, we allocate a 1024-byte buffer and register it as a Memory Region.

### Completion Queue (CQ)
A Completion Queue is an event notification mechanism. When RDMA operations (send, receive, read, write) complete, the results are posted to a Completion Queue. Your application polls or waits on the CQ to learn when operations have finished, whether they succeeded, and the associated metadata. In this project, we create a CQ with capacity for 128 completion entries.

### Queue Pair (QP)
A Queue Pair is the endpoint for RDMA communication. It consists of a Send Queue and a Receive Queue, and is associated with a Completion Queue for notifications. To communicate with a remote peer, you establish a Queue Pair connection between them. Each QP has a state machine (RESET → INIT → RTR → RTS) that must be transitioned through to become operational. In this project, we create a Reliable Connection (RC) Queue Pair, which guarantees ordered, error-checked delivery.

## Test Modes

### Loopback Test (Default)

The program includes a loopback test that demonstrates local RDMA operations on a single Queue Pair:

1. **Setup**: Creates a QP with GID index set to 1 (required for loopback to function correctly)
2. **State Transitions**: The QP automatically transitions through RESET → INIT → RTR → RTS states via the `handshake()` call
3. **Memory Setup**: Allocates and registers 16 bytes of memory (8 bytes for receive, 8 bytes for send)
4. **Send/Receive**: 
   - Posts a Receive Work Request to listen for incoming data
   - Posts a Send Work Request to transmit test data (0x42) to itself
5. **Verification**: Polls the Completion Queue until both operations complete and verifies the data was correctly looped back

This test validates that the RDMA infrastructure is working correctly before attempting remote operations.

### Client-Server Mode

For testing RDMA communication between two machines:

**Server Side:**
1. Auto-discovers RDMA device using RoCEv2 GID (queries network interface mapping)
2. Creates RDMA resources (Protection Domain, Completion Queue, Queue Pair)
3. Listens on TCP port 7471 for client connection
4. Exchanges RDMA endpoint information with client over TCP
5. Establishes RDMA connection via `handshake()` with remote endpoint
6. Posts receive request and waits for data from client
7. Echoes received data back to client

**Client Side:**
1. Auto-discovers RDMA device using RoCEv2 GID
2. Creates RDMA resources
3. Connects to server via TCP
4. Exchanges RDMA endpoint information
5. Establishes RDMA connection with server
6. Sends test data (0x42) to server
7. Receives echo and verifies the data matches

**Key Features:**
- **Automatic Device Discovery**: Queries RDMA device GID table and selects first RoCEv2 GID with valid network interface
- **TCP Handshake**: Exchanges QueuePairEndpoint structures (serialized with bincode) before establishing RDMA connection
- **Single Echo Round**: Client sends once, server echoes once, then both exit (simple verification pattern)
- **Cross-Machine Support**: Works on local loopback (127.0.0.1) or across network interfaces
