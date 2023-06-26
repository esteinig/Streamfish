# Reefsquid

Low-latency adaptive sampling that re-engineers the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api).

## Motivation

I didn't understand enough of the adaptive sampling stack to test ideas and customize adaptive sampling. This was mostly an excercise to re-implement the [`ReadUntil API`](https://github.com/nanoporetech/read_until_api) and parts of the [`Minknow API`](https://github.com/nanoporetech/minknow_api/tree/master/proto/minknow_api) from Oxford Nanopore Technologies (ONT) to learn how `MinKnow` exposes interaction with the pore array through RPC endpoints and how the adaptive sampling queues operate. It turned into a fun re-engineering of some aspects of the `ReadUntil` logic.

[`Icarust`](https://github.com/LooseLab/Icarust) has been a huge help to get started and understand how this can be done in Rust - check out the amazing work by the [Loose Lab](https://github.com/LooseLab) and [@Adoni5](https://github.com/Adoni5).
