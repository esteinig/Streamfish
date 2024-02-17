#!/usr/env/bash

# ====================================
# ICARUST SINGLE EXPERIMENT EVALUATION
# ====================================

# Output directory for Fast5 from Icarust
FAST5_ACTIVE_DIR=$1

# Analysis identifier to create directory
ANALYSIS_ID=$2

# Optional target regions file if targets were provided 
# Must also be provided if taregt string were set in the Streamfish configuration file
TARGET_REGIONS=""

# Output analysis directory for intermediary file outputs and results
ANALYSIS_DIR="/data/storage/streamfish_benchmarks/${ANALYSIS_ID}"

# Reference of sequences used for simulation
SIMULATION_INDEX='/tmp/viruses.mmi'

# Model for Dorado to basecall Fast5 files
DORADO_MODEL='/tmp/models/dna_r9.4.1_e8_fast@v3.4'

# Micromamba shenanigans
micromamba shell init -s bash -p ~/micromamba
source ~/.bashrc
if [ ! -d "$HOME/micromamba/envs/icarust_simulation" ]; then
    micromamba create -f /usr/src/streamfish/scripts/analysis/icarust.simulation.yml -y
fi
# Analysis directory setuo
if [ ! -d "$ANALYSIS_DIR" ]; then
    mkdir $ANALYSIS_DIR
else
    echo "Analysis directory already exists. Overwriting analysis in directory: $ANALYSIS_DIR"
fi

# Analysis start
echo "Running with Fast5 files in: $FAST5_ACTIVE_DIR"

micromamba activate icarust_simulation

dorado basecaller --verbose --emit-fastq ${DORADO_MODEL} ${FAST5_ACTIVE_DIR} > ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.fq 
minimap2 -cx map-ont ${SIMULATION_INDEX} ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.fq > ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.paf

# Gather the unblocked read identifiers
python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py endreason-fast5 --fast5 ${FAST5_ACTIVE_DIR} --output ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.endreasons.csv

if [ ! -d "$TARGET_REGIONS" ]; then
    echo "No target region file provided, using all reference sequence present in the simulation index as targets"
    python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py evaluation \
        --summary-table ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.summary.csv \
        --active-paf ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.paf \
        --active-ends ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.endreasons.csv \
        --plots-prefix ${ANALYSIS_DIR}/${ANALYSIS_ID} \
        --mmi ${SIMULATION_INDEX} \
        --summary-json ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.summary.json
else 
    echo "Target region file provided, using targets as specified in: $TARGET_REGIONS"
    python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py evaluation \
        --summary-table ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.summary.csv \
        --active-paf ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.paf \
        --active-ends ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.endreasons.csv \
        --plots-prefix ${ANALYSIS_DIR}/${ANALYSIS_ID} \
        --target-regions $TARGET_REGIONS \
        --summary-json ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.summary.json
fi



