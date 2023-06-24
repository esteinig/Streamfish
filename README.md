# Reefsquid

Adaptive sampling library that interfaces with `MinKnow` through a custom gRPC client and implements the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) in Rust.

## Motivation

I don't understand enough of the stack to test ideas and customize adaptive sampling. This is mostly an excercise to re-implement the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) and parts of the [`Minknow API`](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) from Oxford Nanopore Technologies (ONT) to learn how `MinKnow` exposes interaction with the pore array through gRPC and how the adaptive sampling queues operate.

[`Icarust`](https://github.com/LooseLab/Icarust) has been a huge help to get started and understand how this can be done in Rust - check out the amazing work by the [Loose Lab](https://github.com/LooseLab) and [@Adoni5](https://github.com/Adoni5).

This is all so cool, learnt all the following things from scratch in the last couple of weeks:

* Asynchroneous programming especially with `tokio` and `tonic`
* Streams and queues
* RPC clients and server details
* UDP/TCP sockets, TLS connections and certificates
* Basic data request/response actions with MinKNOW API
* Basic adaptive samplign logic and re-implementation with async streams
* Basic C++ literacy through Dorado source code
