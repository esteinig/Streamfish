# Streamfish

Low-latency adaptive sampling that re-engineers the [`ReadUntil client`](https://github.com/nanoporetech/read_until_api) using asynchroneous streams and RPC.

## Motivation

I didn't understand enough of the adaptive sampling stack to test ideas and customize applications. This was mostly an excercise to re-implement the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) and parts of the [`Minknow API`](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) from Oxford Nanopore Technologies (ONT) to learn how the control server exposes interaction with the pore array with Remote Call Procedure (RPC) endpoints, as well as how the adaptive sampling queues, caches and decision logic work. 

It turned into a low-latency adaptive sampling client that operates on asynchoneous streams and a custom RPC server (Dori).

[`Icarust`](https://github.com/LooseLab/Icarust) has been a huge help to understand how this can be done in Rust - check out the amazing work by the [Loose Lab](https://github.com/LooseLab) and [@Adoni5](https://github.com/Adoni5).
