# Streamfish <a href='https://github.com/esteinig'><img src='docs/assets/logo.png' align="right" height="200" alt="Giant salmon body-slamming fisherman" /></a>

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
9. [Simulations]()
10. [Benchmarks]()

## Motivation

Streamfish started as a side project to re-implement the [ReadUntil API](https://github.com/nanoporetech/read_until_api) and parts of the [Minknow API](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) in Rust. I wanted to better understand how the adaptive sampling mechanics work in the background. 

While Streamfish approaches the adaptive sampling mechanics from a slightly different angle than [Readfish](https://github.com/LooseLab/Readfish) by using asynchroneous streams, it implements many of the principles developed by the [LooseLab](https://github.com/LooseLab). You could consider Streamfish a highly experimental relative of Readfish.

It is very much recommended **not** to use it for real sequencing runs, unless you are swimming in money or something.

## Documentation

A full description of features, configurations and deployment options can be found [in the documentation]().

## Features

* Low-latency [asynchroneous streaming]() implementation of the adaptive sampling client with `Guppy` and `Dorado`
* [RPC servers]() for adaptive sampling and dynamic real-time assembly with alignment feedback loops
* Slice and dice partitioning of flow cells for high throughput optimization [using multiple clients]() (and experimental configurations)
* Adaptive sampling experiments for depletion, targeted sequencing, coverage balancing and [enrichment for unknown sequences]()
* [Experiment and latency benchmark runners]() with fully-integrated [`Icarust`](https://github.com/LooseLab/Icarust) configurations
* Simulations of complex signal microbiomes with `Squigulator` and `Slow5/Blow5` extensions for `Icarust`

More details on Streamfish features can be found in the documentation.

## Warnings

This is an experimental adaptive sampling client. **It is not user-friendly**. Please use [`Readfish`](https://github.com/LooseLab/readfish) or other suitable implementations for real sequencing experiments.

## System requirements

* Linux system with suitable resources and NVIDIA GPU capable of running Gup[]
* `MinKNOW > v.5.3` for TLS certificates and playback of live sequence runs

I have mainly tested this system on a gaming computer running Ubuntu 20.04 LTS with 16 cores (AMD), NVIDIA GTX 3060 12GB RAM with drivers supporting CUDA 11.4 or higher and 48 GB RAM. Streamfish client and server run their asynchroneous routines on a single thread. Basecalling and reference mapping require more resources depending on throughput and experiment configuration. Slice and dice configurations may be more exhaustive on CPU depending on how many clients are run simultaneously; slice and dice partitions can be spread across multiple GPUs.

## Installation

```

```

## Dependencies

```

```

## Schematics

## Benchmarks

## Simulations

