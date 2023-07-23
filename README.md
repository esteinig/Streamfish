# Streamfish <a href='https://github.com/esteinig'><img src='docs/asesets/logo.png' align="right" height="200" alt="Giant salmon body-slamming fisherman" /></a>

![](https://img.shields.io/badge/version-0.1.0-black.svg)

Low-latency adaptive sampling using asynchroneous streams and RPC.


## Table of contents

1. [Motivation]()
2. [Documentation]()
3. [Features]()
4. [Warnings]()
5. [System requirements]()
6. [Installation]()
7. [Dependencies]()
8. [Schematics]()

## Motivation

Streamfish started as a side project to re-implement the [ReadUntil API](https://github.com/nanoporetech/read_until_api) and parts of the [Minknow API](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) in Rust. I wanted to better understand how the adaptive sampling mechanics work in the background. 

While Streamfish approaches the adaptive sampling mechanics from a slightly different angle than [Readfish](https://github.com/LooseLab/Readfish) it implements many of the principles developed by the [LooseLab](https://github.com/LooseLab). You could consider Streamfish a highly experimental relative of Readfish.

It is very much recommended **not** to use it for real sequencing runs, unless you are swimming in money or something.

## Documentation

A full description of features, configurations and deployment options can be found [in the documentation]().

## Features

Implementation

* Low-latency asynchroneous streaming implementation of the adaptive sampling client
* [RPC servers]() for distributed adaptive sampling decisions and [dynamic feedback loops]()
* `Guppy` and `Dorado` server implementations, [configurable for multiple GPUs]()
* Asynchroneous [`minimap2-rs`]() implementation for reference mapping
* [Slice-an-dice]() partitioning of flow cells for high throughput latency optimization using multiple clients

Experiments

* Adaptive sampling experiments for depletion, targeted sequencing, coverage balancing and unknown sequences
* [Experiment testing]() and [latency optimization runners]() using [`Icarust`](https://github.com/LooseLab/Icarust)
* [Dynamic adaptive sampling feedback loops]() for experimental condition switches and "slow" real-time analysis

## Warnings

This is an experimental adaptive sampling client. **It is not user-friendly**. Please use [`Readfish`](https://github.com/LooseLab/readfish) or other suitable implementations for real sequencing experiments.

## System requirements

* Linux system with suitable resources and GPU (see below)
* `MinKNOW > v.5.3` for TLS certificat and playback or live sequence runs

I have mainly tested this system on a gaming computer running Ubuntu 20.04 LTS with 16 threads (AMD), NVIDIA GTX 3060 12GB RAM with drivers supporting CUDA 11.4 or higher (configured in container) and 48 GB RAM. Streamfish client and server run their asynchroneous routines on a single thread. However, basecalling and reference mapping require some more resources depending on the throughput and experiment configuration you want to run.

## Installation

```

```

## Dependencies

```

```
