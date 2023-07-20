# Streamfish <a href='https://github.com/esteinig'><img src='docs/logo.png' align="right" height="200" /></a>

![](https://img.shields.io/badge/version-0.1.0-black.svg)

Low-latency adaptive sampling using asynchroneous streams and RPC.




## Table of contents

1. [Motivation]()
2. [Features]()
4. [Warnings]()
5. [Requirements]()
6. [Resources]()
7. [Quick start]()
8. [Icarust]()
9. [Modifications]()

## Motivation

I wanted to better understand how the adaptive sampling mechanics work. Streamfish started as a weekend project to re-implement the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) and parts of the [`Minknow API`](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) in Rust (:crab:). 

While Streamfish approaches the mechanics from a slightly different angle than [Readfish](https://github.com/LooseLab/Readfish), it borrows heavily from all the work done by the [LooseLab](https://github.com/LooseLab) - including the super cool dynamic processing loops that feed back changes to the experiment configuration. 

Essentially you can consider Streamfish a highly experimental implementation of Readfish. It is very much recommended **not** to use it for real sequencing runs, unless you are swimming in money or something.

## Features

#### Main:

* Low-latency asynchroneous streaming implementation of the adaptive sampling client
* Relatively stable and tested on long runtimes and high-throughput flowcells
* Latest basecall models with a streaming input for `Guppy` and (unstable) implementation of `Dorado`
* Experiment testing and latency optimization runners using [`Icarust`](https://github.com/LooseLab/Icarust)
* Adaptive sampling experiment presets for depletion, targeted sequencing, coverage balancing and unknown sequences

#### Advanced:

* Partioning of experimental conditions acoss the flowcell and more customizable experiment configurations
* 'Slice-and-dice' multi-client flowcell partitioning for latency optimization and high throughput
* Dynamic adaptive sampling feedback loops for real-time analysis and configuration changes

#### Other:

* Extandable client library mirroring some functions of `ReadUntil API` and `MinKNOW API`
* Read-cache or uncached streaming endpoints, optional throttle for batched actions
* Runs directly on host, in containers, or a mixture of both (latency optimization, development environment)

#### Not implemented:

* Barcode experiments - not needed for my applications, but open to suggestions or pull requests :) 

## Warnings

**Do not use `Streamfish` for real experiments - please use [`Readfish`](https://github.com/LooseLab/readfish) or other suitable implementations!**

This is an experimental version. **It is not user-friendly**.

Compiled binaries, libraries and forks are implemented in the `Docker` images - it *should* not be too difficult to configure and start the containers (😬). However, there may be unanticipated interactions with your supported NVIDIA GPU drivers and CUDA version - this may need to be adjusted in the container image [as described on this page](docs/gpu.md). 

## Requirements

* Linux system with suitable resources and GPU (see below)
* `Docker` and `docker compose` for running the client, servers and simulations
* `MinKNOW > v.5.3` for adaptive sampling playback or sequence runs (maybe don't do those for now)

## Resources

I have mainly tested this system on a gaming computer running Ubuntu 20.04 LTS with 16 threads (AMD), NVIDIA GTX 3060 12GB RAM with drivers supporting CUDA 11.4 or higher (configured in container) and 48 GB RAM. `Streamfish` client and server run their asynchroneous routines on a single thread. However, basecalling and reference mapping require some more resources depending on the throughput and experiment configuration you want to run.

## Quick start

#### Docker production

```bash
# Clone a release version
git clone -b 0.1.0 https://github.com/esteinig/streamfish

# Launch container network
docker compose
```

#### Docker development

Create a `.env` file with a bunch of environmental variables. This mounts development paths and configures the current user during container builds to avoid root writing to mounted paths. It is recommended to make use of `/tmp` on a SSD or NVME drive to ensure fast read/write access to resources.

```
git clone -b 0.1.0 https://github.com/esteinig/streamfish
```

#### No containers (😬)

I outlined a [setup procedure]() to ensure that the correct `protoc` compiler is installed and all components can interact with each other. It's much easier to launch containers due to the number of components and connections required, especially when using the `slice-and-dice` adaptive sampling.

## Icarust

[`Icarust`](https://github.com/LooseLab/Icarust) has been a huge help to understand how this can be done in Rust. It is now fully integrated with `Streamfish` and is preferred over playback runs in `MinKNOW` because strands are replaced after unblocking actions. It therefore allows for experiment simulations with expected outcomes for enrichment or depletions. I also use it to test GPU performance, particularly in environments where it is not safe to run playbacks on `MinKNOW` (due to its annoying root access). 

Check out the amazing work by [@Adoni5](https://github.com/Adoni5) and [Loose Lab](https://github.com/LooseLab). Please also cite [their preprint](https://www.biorxiv.org/content/10.1101/2023.05.16.540986v1) if you - for whatever arcane reason - use `Streamfish` in your publication (not recommended at this stage).

## Modifications

Modifications to tools used in `Streamfish` to make this work:

1. TLS certification checks: deactivated certification checks in the underlying `tonic v0.9.2` library as they were incompatible with the `MinKNOW` certificate version
2. Dorado streaming input: added a `DataLoader` method that allows reading a text based input stream that contains the uncalbriated signal arrays and device configurations for basecalling
3. Dorado batch timeout: added a command line option (`--batch-timeout` in microseconds) that allows setting the timeout before an incomplete batch is launched for basecalling - this is necessary to improve latency due to inpout streams and launches models as quickyl as possible
4. Icarust quality of life: added delay and timeout to Icarust for standardizing benchmark experiments, added optional actions (as in MinKNOW) for experiment control testing, added channel size on `GetLiveReadsRequest` setup configuration to get channel subsets only and allow for `slice-and-dice` runs on multiple GPUs
