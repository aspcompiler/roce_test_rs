# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust project for testing ROCE (RDMA over Converged Ethernet) using soft-RoCE (Soft Remote Direct Memory Access over Converged Ethernet). The project demonstrates basic RDMA operations through the `ibverbs` Rust bindings, including device discovery, protection domains, memory registration, completion queues, and queue pairs.

## Prerequisites and Setup

The project requires RDMA capabilities. On Ubuntu 24.04:

```bash
sudo apt update
sudo apt install linux-modules-extra-$(uname -r)
sudo apt install -y rdma-core ibverbs-utils rdmacm-utils perftest build-essential cmake libibverbs-dev pkg-config python3-dev clang libclang-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

To load soft-RoCE and bind it:

```bash
sudo modprobe rdma_rxe
ip addr  # identify your Ethernet interface (e.g., eth0)
sudo rdma link add rxe0 type rxe netdev eth0
rdma link  # verify the link is up
ibv_devinfo  # verify the RDMA device is visible
```

The program expects the soft-RoCE device to be named `rxe0`.

## Common Commands

- **Build**: `cargo build` or `cargo build --release`
- **Run**: `cargo run` (requires soft-RoCE to be loaded and bound; may need `sudo`)
- **Run with release optimizations**: `cargo run --release`
- **Format code**: `cargo fmt`
- **Lint**: `cargo clippy`

## Architecture

The codebase is minimal and single-file:

- **src/main.rs**: Demonstrates a complete RDMA workflow:
  1. Device discovery (finds the `rxe0` soft-RoCE device)
  2. Device context creation
  3. Protection Domain allocation
  4. Memory allocation and registration (1024-byte buffer)
  5. Completion Queue creation
  6. Queue Pair creation (Reliable Connection type)

The program serves as both a functional test and an educational reference for RDMA operations using the `ibverbs` crate.

## Key Dependencies

- **ibverbs** (0.9.2): Rust bindings for InfiniBand Verbs API, providing low-level RDMA abstractions

## Notes

- The code uses unwrap() for error handling in several places; this is acceptable for a test program but production code should use proper error handling.
- Soft-RoCE is a kernel-space implementation of RoCE over regular Ethernet; it does not require specialized hardware but is slower than hardware RoCE.
- The program allocates and registers 1024 bytes of memory as a demonstration; actual workloads would allocate based on their needs.
