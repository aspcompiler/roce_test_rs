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
