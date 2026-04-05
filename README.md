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

cargo run

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
