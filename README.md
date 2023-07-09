# Streamfish

Low-latency adaptive sampling that re-engineers the [`ReadUntil client`](https://github.com/nanoporetech/read_until_api) using asynchroneous streams and RPC.

## Motivation

I didn't understand enough of the adaptive sampling stack to customize applications. This started as an excercise to re-implement the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) and parts of the [`Minknow API`](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) from Oxford Nanopore Technologies (ONT) to learn how the control server and Remote Call Procedure (RPC) endpoints work, as well as how the adaptive sampling queues, caches and decision logic operate. 

It turned into a fun and slightly crazy project designing and testing a low-latency adaptive sampling client that operates on asynchoneous streams and custom RPC servers, and is - of course - fully implemented in Rust (with some C++ modifications of [`Dorado`](https://github.com/esteinig/dorado/commits/dori-stdin-v0.3.1)) 🦀.

## Warnings

This is an early development version that (somewhat surprisingly) works. **It is not user-friendly.** At the very least you will need to be familiar with using `Docker` and willing to spend some time adjusting `Dorado` on your GPU. 

Compiled binaries, libraries and forks are implemented in the `Docker` images - it **should** not be too difficult to configure and start the containers (😬). However, there may be unanticipated interactions with your supported NVIDIA GPU drivers and CUDA version - this may need to be adjusted in the container image [as described on this page](docs/gpu.md). 

**Do not use `Streamfish` for real experiments - please use [`Readfish`](https://github.com/LooseLab/readfish) or other suitable implementations!**

## Requirements

* Linux system with suitable resources and GPU (see below)
* `Docker` and `docker compose` for running the client, servers and simulations
* `MinKNOW > v.5.3` for adaptive sampling playback or sequence runs (maybe don't do those for now)

## Resources

I have tested this system on a basic gaming computer running Ubuntu 20.04 LTS with 16 threads (AMD), NVIDIA GTX 3060 12GB RAM with drivers supporting CUDA 11.4 or higher (configured in container) and 48 GB RAM. 

Empirically `Streamfish` client and servers do not need a lot of resources. If you are using a human genome for reference alignment you may need 24-32 GB of RAM - initially my system ran out with 16GB while also running `MinKNOW`. 

When testing high-throughput setups with `Icarust` (> 512 pores) you will need to adjust Dorado batch size in the `Streamfish` configuration - there is a few caveats around batch sizes and too low a batch size may cause a "slipstream" behaviour before `Dorado` crashes without informative error messages. I have documented the investigation and debugging steps [here](https://github.com/esteinig/Streamfish/issues/18) and written some recommendation on what to look out for and how to adjust batch sizes for expected throughput [on this page](docs/gpu.md). High-throughput setups (> 2048 pores) with suitable batch sizes require ~ 10GB GPU RAM.

## Icarust

[`Icarust`](https://github.com/LooseLab/Icarust) has been a huge help to understand how this can be done in Rust. It is now fully integrated with `Streamfish` and is preferred over playback runs in `MinKNOW` because strands are replaced after unblocking actions. It therefore allows for experiment simulations with expected outcomes for enrichment or depletions. I also use it to test GPU performance, particularly in environments where it is not safe to run playbacks on `MinKNOW` (due to its annoying root access). 

Check out the amazing work by [@Adoni5](https://github.com/Adoni5) and [Loose Lab](https://github.com/LooseLab). Please also cite [their preprint](https://www.biorxiv.org/content/10.1101/2023.05.16.540986v1) if you should - for whatever reason - use `Streamfish` in your publication (not recommended at this stage).


