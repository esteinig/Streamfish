#/bin/bash

# Experimental data series 1:
#
#   - System latency measurements at checkpoints and across configuration settings
#   - Simple community simulations (bacterial, host-viral, parasite) (dynamic, not dynamic, with delay or without, controls, replicates with different seeds)
#   - Show that different experimental conditions work (vs. Readfish baseline): enrichment/depletion of community targets, enrichment of host genome panels
#   - Controls and signal/basecalls of condition vs no condition - measure total enrichment/depletion vs control
#   - Measure effect of simulated signal library parameters: mean read length, chemistry and platform
#   - Measure effect of experimental conditions: reference database composition (relatedness, see levelled data approach from Ryan)
#   - Optimize system compoinents like read chunk vs stream at lower break radius, spreading resources, slice and dice from PromethION


# Data preparation
# ================

# Generate the baseline datasets with `cipher` and `squigulator` 1 GB MinION R10.4.1, 5 Kbp mean read length at different pathogen : background proportions (bacterial: S. aureus, viral: human host)

SRC="$PWD"
THREADS=16
REPLICATES=1
OUTDIR="$PWD/experiments/part1"

mkdir -p $OUTDIR

for bacterial_config in "$SRC/manuscript/part1/configs/cipher/bacterial/dna_minion/*.toml"; do
    
    NAME=$(basename $bacterial_config .toml)
    DATA_DIR="$OUTDIR/data/bacterial/dna_minion/$NAME" 

    if [ -d $DATA_DIR ]; then
        echo "Data directory exists for >> $NAME << experiment series 1, skipping simulation..."
        continue
    fi

    echo "Simulating community >> $NAME << experiment series 1..."
    cipher simulate-community --config $bacterial_config --outdir $DATA_DIR --overflow 0.1 --threads $THREADS
done


for bacterial_config in "$SRC/manuscript/part1/configs/cipher/viral/dna_minion/*.toml"; do

    NAME=$(basename $bacterial_config .toml)
    DATA_DIR="$OUTDIR/data/bacterial/dna_minion/$NAME" 
    
    if [ -d $DATA_DIR ]; then
        echo "Data directory exists for >> $NAME << experiment series 1, skipping simulation..."
        continue
    fi

    echo "Simulating community >> $NAME << experiment series 1..."
    cipher simulate-community --config $bacterial_config --outdir $DATA_DIR --overflow 0.1 --threads $THREADS --prefix $NAME
done


# ============
# Experiment 1
# ============

# Run viral and bacterial adaptive sampling simulations across unblock-all Streamfish configurations:
#
#   - Measures system latency at client, RPC server, basecaller, and mapping stages
#   - Benchmark basecaller models latency introduction, package loss of UDS vs TCP connections
#   - Impact of read cache size and scale up to large pore arrays (impact on latency)
#   - Slice and dice latency optimization on large throughput in Daedalost fork,
#     flowcell partitioning across multiple clients and GPU, runner in Streamfish


# Same model and basecaller config (MinION, PromethION, guppy-basecall-server, dorado-basecall-server,
# R10 e8.2 fast, hac, sup), 512 and 3000 pores at 90% starting occupancy with pore death model 
# from Icarust configured by target yield 1 GB  where pore death is noticeable over runtime with 
# fixed limit (30 mins), same database index of simulated genomes for minimap2-rs


for read_until_config in "$SRC/manuscript/part1/configs/streamfish/experiment1/*.toml"; 
    i = 0
    for DATA_DIR in "$OUTDIR/data/bacterial/dna_minion/*/"; do # DATA DIR names e.g. test_1, test_2 need paralell iter
        BLOW5="$DATA_DIR/*.blow5"
        GPU="cuda:${i}"
        COMMUNITY_NAME=$(basename $DATA_DIR)
        streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/experiment --prefix $REP --seed 0 --gpu $GPU
        streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/control --prefix $REP --seed 0 -- $GPU --control  
        i += 1
    done
done


# ============
# Experiment 2
# ============

# Run viral and bacterial adaptive sampling simulations across 4 initial Streamfish configurations for targeted sequencing of 
# pathogen community member (Mycobacterium and VZV) total 1800 seconds runtime across a siulated run yield of 1 GB: 
#
#   - Dynamic target switch on or off (3 mins interval)
#   - Data generation sleep as break chunks (400 ms)
#   - Control runs without target (--control true)
#   - Technical replicates with different seed values of simulated community sampler (--seed 0)

# Same model and basecaller config (MinION, guppy-basecall-server R10 e8.2 fast), 512 pores at
# 90% starting occupancy with pore death model from Icarust configured by target yield 1 GB 
# where pore death is noticeable over runtime with fixed limit (30 mins)

# Minimum time required: 2 x 8 x (2 x 2 x 2 x 3) = 384 / 2 = 192 hours at 30 minutes runtime / 8 GPU on DGX in parallel = 24 hours runtime

# Run a positive simulation of Icarust only (no adaptive sampling control) and one where all targets are mapped against (positive mapping control, delta is due to mapping efficiency all else being equal)

for REP in 1..$REPLICATES; do
    for read_until_config in "$SRC/manuscript/part1/configs/streamfish/experiment2/*.toml"; 
        i = 0
        for DATA_DIR in "$OUTDIR/data/bacterial/dna_minion/*/"; do # DATA DIR names e.g. test_1, test_2 need paralell iter
            BLOW5="$DATA_DIR/*.blow5"
            GPU="cuda:${i}"
            COMMUNITY_NAME=$(basename $DATA_DIR)
            streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/experiment --prefix $REP --seed 0 --gpu $GPU
            streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/control --prefix $REP --seed 0 -- $GPU --control  
            i += 1
        done
    done
done


for REP in 1..$REPLICATES; do
    for read_until_config in "$SRC/manuscript/part1/configs/streamfish/experiment2/*.toml"; do
        i = 0
        for DATA_DIR in "$OUTDIR/data/viral/dna_minion/*/"; do # DATA DIR names e.g. test_1, test_2 need paralell iter
            COMMUNITY_NAME=$(basename $DATA_DIR)
            BLOW5="$DATA_DIR/${COMMUNITY_NAME}_community.blow5"
            GPU="cuda:${i}"
            streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/experiment --prefix $REP --seed --gpu $GPU
            streamfish read-until --config $read_until_config --simulation $BLOW5 --outdir $OUTDIR/data/viral/simulations/$COMMUNITY_NAME/control --prefix $REP --seed 0 -- $GPU --control  
            i += 1
        done
    done
done


# ============
# Experiment 3
# ============

# Run viral and bacterial adaptive sampling simulations across 4 initial Streamfish configurations for targeted sequencing of 
# pathogen community member (Mycobacterium and VZV) total 1800 seconds runtime across a simulated run yield of 1 GB: 
#
#   - Which basecaller, model and average read length is best for enrichment tasks (latency, community simulations) 
#   - Effect of read cache and mapping performance - database size, composition, resources, streaming config vs cached chunk logic 
#   - What matters for the database composition - closely related vs comprehensive vs targeted

#   - Unknown viral database testing - holdouts on viral database genomes to test unknown sequence enrichment
#   - Complex community background simulations of "realistic" clinical samples
#   - Real flowcell testing on mock communities of Staphylococcus and VZV (read length optimized extractions) and bog sample environmental for targeted or unknown taxa (Calum)

#  MinION or PromethION guppy-basecall-server and dorado-basecall-server, R10 e8.2 fast, hac, sup, 
#  512 and 3000 pores at 90% starting occupancy with pore death model from Icarust configured by target yield 1 GB or 10
#  where pore death is noticeable over runtime with fixed limit (30 mins)

# Minimum time required: 2 x 8 x (2 x 3 x 3) = 288 / 2 = 144 / 8 = 18 hours runtime for basecaller and length thresholds
# Minimum time required: 2 x 8 x (2 x 3 x 3) = 288 / 2 = 144 / 8 = 18 hours runtime for read cache and mapping performance



# ============
# Experiment 4
# ============

# Run viral and bacterial adaptive sampling simulations across Streamfish configurations for targeted sequencing of 
# pathogen community member (Mycobacterium and VZV) total 24 hour runtime across a simulated run yield of 50 GB: 
#
#   - Scalability to high throughput simulations - effect on latency and classification performance in base simulations
#   - Dynamic feedback loop of real-time seed assembly feeding back contig to the experimental conditions and optimize unknown taxa
#   - Agent based flow-cell partitioning and dynamic experimental switches for targeted sequencing
#   - Dynamic feedback loop for target coverage optimization based on simulations (BOSS implementation)

#  PromethION guppy-basecall-server, R10 e8.2 fast 3000 pores at 90% starting occupancy with pore death model from Icarust 
#  configured by target yield 60 GB, multiple A100 GPUs with multiple clients.

