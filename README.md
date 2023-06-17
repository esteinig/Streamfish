# Reefsquid

Adaptive sampling library that interfaces with MinKnow through gRPC and implements the ReadUntil API in Rust.

## Motivation

I don't understand enough of the underlying stack to customize adaptive sampling for testing some ideas. This is mostly an excercise to re-implement the ReadUntil API from Oxford Nanopore Technologies to learn how the gRPC server from MinKnow exposes operations on the pore array. `Icarust` has been a huge help to understand how this can be done in Rust.
