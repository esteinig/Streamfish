# Overview

Welcome to the `Streamfish` documentation. Installation and configuration of the client are described in the following sections.

## Background

`Streamfish`, `MinKNOW` and `Icarust` work with the [remote procedure call](https://en.wikipedia.org/wiki/Remote_procedure_call) protocol ([gRPC](https://grpc.io/)). 

If you want to know more about the mechanics of `Streamfish` have a look at the [background mechanics tutorial](#tutorial.md#mechanics). It may be useful for understanding how parameter configurations influence the performance of adaptive sampling experiments.

## Requirements

`Streamfish` operates on `Linux` operating systems - it has not been (and will likely not be) tested on `Windows` or `MacOS`. We provide `Docker` images and networked containers that in principle can be run on these operating systems and do not introduce significant latency (when deployed on Linux).


!!! info

    If you are comfortable with `Docker` it is recommended to use the provided [images]() and [`docker compose`]() as described in the tutorial [Setting up with Docker and container networks](tutorials.md#docker). Containers automatically handle system dependencies and some of the NVIDIA CUDA toolchain.


## Installation

You may need to install some system-wide dependencies to get started. I will outline these dependencies as part of the different ways to install `Streamfish`. Some of these dependencies are also required for `Icarust`. You can catch two fish with one... lure? 

### Anaconda

Easiest setup is through the [`CondaForge`]() and [`BioConda`]() distribution systems.

The environment is setup manually to compile `Streamfish`. This will eventually be replaced with a package on `BioConda`.

```bash
# Set to a directory in your $PATH
DIRECTORY=/usr/src/streamfish/bin

# Clone the repository to get the environment file
git clone https://github.com/esteinig/streamfish

# Setup environment and install dependencies
micromamba env create -f streamfish/conda.yml

# Activate environment
micromamba activate streamfish

# Compile the Rust client
cd streamfish && cargo build --release

# Move the executable to a $DIRECTORY in your $PATH
mv target/release/streamfish $DIRECTORY

# Test the client
streamfish --help
```


### Docker

Assumes the following setup:

* NVIDIA GPU
* `Docker` with `docker compose` and `nvidia-docker`
* Current user in the `Docker` user group


```


```


### Manual

Assumes `sudo` access for user


```bash
# Install system dependencies using `apt install` - this may be different on your distribution
sudo apt update && sudo apt install -y \
    git make cmake musl-tools \
    libprotobuf-dev protobuf-compiler \
    libhdf5-dev libzstd-dev

#

```


Building the RPC server and clients requires `protobuf` compiler. I noticed that on older operating system versions including Ubuntu 20.04 LTS the compiler version on system-wide installation does not support some features required by the `MinKNOW` RPC specification. It is safest to install the compiler manually like so:

```bash

```