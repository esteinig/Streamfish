<!-- 
<p align="left" style="margin-top: 2rem; margin-bottom: 2rem">
    <img src="assets/logo.png#only-light"  width="300" height="200">
    <img src="assets/logo.png#only-dark" width="300" height="200">
</p> -->

Streamfish is a low-latency adaptive sampling client that implements asynchroneous routines and streaming operations via remote procedure call servers (RPC). It aims to
provide a fast and scalable implementation of adaptive sampling for experimental applications.


!!! warning

    `Streamfish` is highly experimental and should probably not be used for live sequencing runs at this stage



## Overview

* [Documentation](streamfish/index.md)  
Installation and configuration of `Streamfish`

* [Experiments](experiments/index.md)  
Adaptive sampling experiment configurations

* [Benchmarks](benchmarks/index.md)  
Benchmarks and other shenanigans with `Icarust`

* [Tutorials](experiments/index.md)  
Tutorials for advanced configurations and experiments

* [Dependencies](benchmarks/index.md)  
Description and citations of tools used in Streamfish

## Schematic

Working schematic of the Streamfish client


## Features


## Citations

If you use Streamfish for research please cite:

> Steinig (2023) - Streamfish: low-latency adaptive sampling using asynchroneous streams

Streamfish extensively uses [Icarust](https://github.com/LooseLab/Icarust) for testing:

>  Rory Munro, Alexander Payne, Matthew Loose (2023) - Icarust, a real-time simulator for Oxford Nanopore adaptive sampling - [bioRxiv](https://doi.org/10.1101/2023.05.16.540986 )
