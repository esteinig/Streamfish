# Low-latency adaptive sampling client

In this section of the manuscript, we improve the adaptive sampling client and proviude a low-latency optimization that allows us to implement custom features and dynamic feedback loops. The main aims of this section are:

1. Re-engineer and describe the low-latency adaptive sampling client `Streamfish`
2. Identify contributions to latency in client implementations using experimental simulations in `Icarust`
2. Evaluate signal community evaluations and  simulations in `Icarust`

## Part 1 - Streamfish architecture, modes of operation and system latency evaluation with `unblock-all`

We describe the re-implementation of an adaptive sampling client optimized for low-latency operation in Rust. 

We describe latency optimisation experiments using playback runs and simulations. `Readfish` (Payne et al. 2021) is used as a reference for comparison.

### Overview

In this section, we use the `unblock-all` feature of `Streamfish` to identify contributions to latency arising from architecture components and design choices, as well as experimental conditions of the sequencing run itself. We compare these to the `unblock-all` feature of `Readfish`. Unblocking a read is the terminology used for sending an action for a specific pore (channel) back to the control server, which then unblocks the specific pore (channel) by reversing its polarity and ejecting the read that is translocating through the pore. `Unblock-all` is a feature that, regardless of basecaller or classification outcome, sends back an unblock decision for every signal chunk received from the control server, thus rejecting and truncating all reads as fast as possible. 

The `Readfish` `unblock-all` configuration receives a batch of signal chunks per channel and immediately sends a batch of unblock actions to the control server without basecalling or alignment (https://github.com/LooseLab/readfish/blob/dev_staging/ru/unblock_all.py). We implement a comparable `unblock-all-client` feature (Figure 1, green circuit), where a batch of signal chunks is received and an action is sent back for each chunks, rather than a single batched action, reflecting the inherent streaming mode of operation in `Streamfish`. Theoretically, this puts `Streamfish` at a slight disadvantage by sending multiple (streamed) actions in sequence, instead of a batch of actions contained in a single response to `MinKNOW`.

Because we are also using an additional processing server (Dori, Figure 1) we additionally implement an `unblock-all-server` mode (Figure 1, blue circuit) where signal chunks are streamed to Dori and a response streamed back to the client without processing the read. This is followed by sending an unblock action for each chunk to the control server. Moreover, because we are using a threaded standard input/output stream for passing signal chunks to the basecaller `Dorado` we implement an `unblock-all-process` mode, which additionally transforms the signal byte arrays into uncalibrated signal integer arrays.These are passed into a modified `Dorado` or into a replacement program (C++) that implements the same standard input processing loop as our `Dorado` fork (https://github.com/esteinig/dorado/tree/dori-stdin-v0.3.1), but does not transform signal arrays into tensors or sends them into the basecalling nodes. Instead, channel and read number sent into the standard output stream of the program and parsed in the endpoint response stream that usually parses the alignment outputs on Dori, sending back an unblock decision for every signal chunk "processed". Overall, this allows us to measure contributions of latency arising from passing the signal data through the input/output stream of an external program.

### Latency

Latency is measured outcome-based - our primary metrics are the mean and median read length of unblocked reads, which are computed from the output signal data basecalled with the same basecaller and model for each experimental setup. Unblocked read lengths were chosen over time-based measurements, as it allowed us to compare across control runs and the `Readfish` `unblock-all` configuration. It also produces a tangible and interpretable measure that is understood intuitively and can be visualized as read length distributions, whereas read throughput in microseconds is harder to interpret overall. However, we provide detailed time-based measurements in [Supplementary Data 1](#supplementary-data).

First, we establish a base-line of `unblock-all` testing during a play-back run of a commonly used adaptive sampling evaluation experiment described initially for testing `Readfish` (https://github.com/LooseLab/readfish) and used by other client implementation validations like `ReadBouncer` (Ulrich et al. 2022) (which does not provide an `unblock-all` configuration for testing and could therefore not be used as reference without substantial modification of source code that would bias the comparisons). We then compare the baseline measurement to simulatiuons in `Icarust` where we note a conceptual error in the implementation of the important `break_reads_after_seconds` configuration adopted from `MinKNOW`. After fixing this error and demonstrating parity between play-back and simulated runs in unblock-all testing, we continue to use `Icarust` simulations to demonstrate latency contributionsfrom different components, configurations and sequencing conditions with `Streamfish`.

### Configurations

**Host and Docker**: We ran all tests on a standard gaming computer with 48GB RAM, 16 threads (AMD Ryzen 5) and a GTX 3060 12MB, running Ubuntu 20.04 OS. For the main evaluation `MinKNOW` and `Icarust` control servers, and `Readfish` and `Streamfish` ran directly on the host.  Clients connected to control servers through a TLS encrypted TCP channel with token authentication as required by `MinKNOW`. `Dori` ran inside a Ubuntu 20.04 OS Docker container with NVIDIA configuration and appropirate CUDA version. It connected to the control serve rto obtain experiment configuration like digitisation and sample rate through host ports forwarded into the container and on the required TCP connection. `Streamfish` connected to `Dori` on a Unix Domain Socket (UDS) channel which was located on the NVME drive of the host mounted inside the container.

```zsh
docker compose -f docker/docker-compose.yml --profile dori --project-name unblock-all --env-file docker/.env up
```

**Alternative**: In this setup we ran `Icarust` as well as the `Streamfish` or `Readfish` clients in the same Debian 12.0 container. `MinKNOW` ports were forwarded to the container from the host, and a connection to `Dori` was made through the UDS mounted in both the `Dori` and the `Streamfish` container. Connections to `Dori` can also be made with an unsecured TCP channel on a shared Docker network or through forwarding ports from the `Dori` container to the host, and ports from the host into the `Streamfish` container.


```zsh
docker compose -f docker/docker-compose.yml --profile streamfish --project-name unblock-all --env-file docker/.env up
```

`Streamfish` was compiled with the `--release` flag using the `x86_64-unknown-linux-musl` toolchain and `Rust v1.69`. Due to instability of a dependency, `Icarust` was compiled with the `--release` flag using the default toolchain and `Rust v1.69`.


**MinKNOW control server**: configuration of `MinKNOW` includes the `Readfish` recommended change in the control server configuration file from `break_reads_after_seconds=1.0` (1.0 seconds) to `break_reads_after_seconds=0.4` (0.4 seconds). This parameter controls the size of the signal chunks that are sent from the `MinKNOW` control server endpoint `GetLiveReads`. At a sample rate of 4000 Hz, each chunk corresponds to 1600 ADC values (analogue-to-digital converter values) which are the uncalibrated (raw) signal values sampled from the the device with `MinKNOW`. This translates to 184 bp reads using the default 460 bases per second (bp/s) translocation speeds during a sequening run (R9.4.1). 

Therefore, in the ideal case of no latency occuring between receipt of signal chunks and unblocking a read, we would expect to observe reads lengths distributed around a mean of 184 bp in the `unblock-all` configuration with some variation in the actual length of the signal chunks (200 ADC minimum, as configured in the setup request to `MinKNOW`) and from basecaller variation between models and underlying neural architectures e.g. Guppy vs Dorado (Figure 2). Each additional base sequenced then corresponds to an observed latency of 2.17 milliseconds and is the result of latency in the client architecture and data transmission, as we are ignoring basecalling and classification stages.

We further configure the control server using the setup action request and set an unblock duration of 100 milliseconds (default in `Readfish`), return data on all channels across the array (512 for MinION, 2048 for PromethION) and specify a minimum of 200 ADC values per sample of the device signal stream that makes up the signal chunks of 1600 ADC values returned from the control server. We use the aforementioned play-back run from a R9.4.1 MinION flowcell running a human LSK110 library (DNA). We note that pore occupancy varies between approximately 64 - 128 actively sequencing pores and therefore constitute low-throughput experimental conditions on a MinION. Live basecalling with `guppy_server` was active during the run with default configurations. Outputs were set to `.fast5` signal files to harmonize with the output format currently supported in `Icarust`.

**Icarust control server**: configuration of the `Icarust` simulation mirrors those of the initital `MinKNOW` play-back run. We retained the default `break_reads_after_seconds` equivalent parameter `break_reads_ms=400` (0.4. seconds) and set the channel sequencing to 512 to emulate a MinION flowcell. In addition, we set `working_pore_percent=25` to simulate a low-occupancy flowcell comparable to the play-back run in `MinKNOW`. The experiment we configured was a two-bacterial default experiments using the R9.4.1 squiggle arrays provided for *Pseudomonas aeruginosa* () and *Bacillus anthracis* () which we implemented with 4000 bp mean read length distributions. Since all reads are immediately unblocked and our evaluation considered only latency comparisons of unblocked reads (no basecalls, alignments, decision logic implemented) the chosen organism was not of relevance, except to provide sufficient mean read lengths for unblock calls to occurr in case of high latency. We describe additional `Icarust` configurations for in-depth analysis in the benchmark experiments section, and refer to this configuration only in the initial comparison to `MinKNOW`.


**Readfish client**: we used the default 



**Streamfish client**:


### Results


#### Initial comparison between `MinKNOW` and `Icarust`

1. Main justification for modification of `Icarust` - will need to be a PR for Rory (and to get their opinion)

#### Baseline 

1. Experiment baselines run to show that long-running unblocked distributions are equivalent to short-running ones - justifies using short test runs for parameter optimization and exploration.

#### Preliminary

1. Throughput and how to scale - compare with unblock-all and identify bottlenecks in latency
2. Read length + proportions of strains
2. Basecaller optimization - UDS and client threads seems to be important, no throttle on results also seems to be important at higher throughputs, num caller interaction with dataset? Also! It seems cached size for the unknown sample experiment is critical - longer stretches on proceed will eventually map against homologous regions
3. Experiment type - reference size and runnign actual experiment vs. unblock-all, interactions with throughput and basecaller, impact on logic / decision matrix on latency
4. Impact on mapper - single threaded (can we even do multithread)

Mean length unblocked is influenced by max chunk size cached - logner reads are allowed to iterate through the classification loop (few) - median length is more constant and relflects the weight of the center of gravity for the unblock read length distribution.

Slice and dice rescues latency (effect somewhat mild, but may be more pronounced at longer run times - compare with single thread, looks like we reach limits imposed mainly by basecallling and maybe mapping - need to investigate)

Compare max read chunks (allowign longer unblocks) with the gain in unblocked percent of target and ratio of target/non-target - looks like lower read chunks might be more effective than allowing larger one, despite lower percentage unblocked of target.

Compare unblock all settings on range of test datasets and experiment combinations - small reference (known, e.g. two viruses), large reference (known, human), medium reference but many sequences (known, viral database)


### Integration with `Icarust` fork and `Cipher` community simulation benchmarks




## Supplementary Data
