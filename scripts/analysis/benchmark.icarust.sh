#!/usr/env/bash

# ====================================
# ICARUST SINGLE EXPERIMENT EVALUATION
# ====================================

# Base output directory for the benchmarks
BASE_DIR=$1
BASE_NAME=$(basename $BASE_DIR)

# Reference of sequences used for simulation
SIMULATION_INDEX='/tmp/chm13v2_chr1_chr2.mmi'

# Model for Dorado to basecall completed Fast5 files from Icarust simulations
# this must be kept constant for parity in the evaluation metrics
DORADO_MODEL='/tmp/models/dna_r9.4.1_e8_fast@v3.4'


# Output analysis directory for intermediary file outputs and results
ANALYSIS_DIR="/data/storage/streamfish_benchmarks/${BASE_NAME}"

# Analysis directory setup
if [ ! -d "$ANALYSIS_DIR" ]; then
    mkdir $ANALYSIS_DIR
else
    echo "Analysis directory already exists. Overwriting analysis in directory: $ANALYSIS_DIR"
fi

# Micromamba shenanigans
micromamba shell init -s bash -p ~/micromamba
source ~/.bashrc
if [ ! -d "$HOME/micromamba/envs/icarust_simulation" ]; then
    micromamba create -f /usr/src/streamfish/scripts/analysis/icarust.simulation.yml -y
fi

micromamba activate icarust_simulation


for GROUP_DIR in $BASE_DIR/*/; do
    GROUP_NAME=$(basename $GROUP_DIR)
    echo "Analysing benchmark group: $GROUP_NAME"

    for BENCHMARK_DIR in $GROUP_DIR/*/; do
        BENCHMARK_NAME=$(basename $BENCHMARK_DIR)
        echo "Analysing benchmark: $BENCHMARK_NAME"
        mkdir -p "$ANALYSIS_DIR/$GROUP_NAME"
        

        # Analysis start
        FAST5_DIR="$BENCHMARK_DIR/fast5/fast5_pass"
        echo "Analysing Fast5 outputs in: $FAST5_DIR"

        PREFIX="$ANALYSIS_DIR/$GROUP_NAME/$BENCHMARK_NAME"

        dorado basecaller --verbose --emit-fastq ${DORADO_MODEL} ${FAST5_DIR} > ${PREFIX}.fq 
        minimap2 -cx map-ont ${SIMULATION_INDEX} ${PREFIX}.fq > ${PREFIX}.paf

        # Gather the unblocked read identifiers
        python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py endreason-fast5 --fast5 ${FAST5_DIR} --output ${PREFIX}.endreasons.csv

        echo "No target region file provided, using all reference sequence present in the simulation index as targets"
        python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py evaluation \
            --summary-table ${PREFIX}.summary.csv \
            --active-paf ${PREFIX}.paf \
            --active-ends ${PREFIX}.endreasons.csv \
            --plots-prefix ${PREFIX} \
            --mmi ${SIMULATION_INDEX} \
            --summary-json ${PREFIX}.summary.json

    done

done



