# Reefsquid

Adaptive sampling library that interfaces with MinKnow through a custom gRPC client and implements the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) in Rust.

## Motivation

I don't understand enough of the underlying stack to test some ideas and customize adaptive sampling. This is mostly an excercise to re-implement the `ReadUntil API` from Oxford Nanopore Technologies to learn how MinKnow exposes operations on the pore array through gRPC. `Icarust` has been a huge help to understand how this can be done in Rust - thanks to [@Adoni5](https://github.com/Adoni5) and the [Loose Lab](https://github.com/LooseLab).
