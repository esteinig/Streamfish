#!/usr/env/bash

# =============================
# ICARUST EXPERIMENT EVALUATION
# =============================

# Run on host - uses containers to execute basecalling and other specific tasks. The following steps
# require the container network to be configured and executed in development mode, for example:

# docker compose -f docker/docker-compose.yml --profile dev --project-name main --env-file docker/.env up

# Enter the evaluation container and execute the script on the correct input paths and settings (see below)

# =============================
# STEP 1: DORADO BASECALLING
# =============================

CONTROL=1

ANALYSIS_DIR="/tmp/streamfish_analysis_${ANALYSIS_ID}"

MAPPING_REFERENCE='/tmp/bacteria.mmi'

ANALYSIS_ID=$1
FAST5_ACTIVE_DIR=$2

FAST5_CONTROL_DIR="/tmp/test_bacteria/test/20230711_0611_XIII_FAQ12345_5ef1fa3b9/fast5_pass/"

DORADO_FST_MODEL='/tmp/models/dna_r9.4.1_e8_fast@v3.4'

# DORADO_HAC_MODEL='/tmp/models/dna_r9.4.1_e8_hac@v3.3'
# DORADO_SUP_MODEL='/tmp/models/dna_r9.4.1_e8_sup@v3.3'

# Always initialize shell - failure to activate after starting new container otherwise
micromamba shell init -s bash -p ~/micromamba
source ~/.bashrc

if [ ! -d "$HOME/micromamba/envs/icarust_evaluation" ]; then
    micromamba create -f /usr/src/streamfish/scripts/analysis/evaluate.icarust.yml -y
fi

if [ ! -d "$ANALYSIS_DIR" ]; then
    mkdir $ANALYSIS_DIR
else
    echo "Analysis directory already exists. Overwriting analysis in directory: $ANALYSIS_DIR"
fi

echo "Running analysis on: $FAST5_ACTIVE_DIR"

micromamba activate icarust_evaluation

# Run the Dorado basecaller with FAST, HAC and SUP models on the Fast5 outputs from Icarust; use the same 
# aligner and settings as implemented in Streamfish [test version with Dorado]. Batchsize is automatically
# chosen and batch timeout is reset to the default setting (100 milliseconds as in Dorado v0.3.1)
dorado basecaller --verbose --batchsize 0 --reference ${MAPPING_REFERENCE} -k 15 -w 10 -g 16 --batch-timeout 100000 --num-runners 2 --emit-sam ${DORADO_FST_MODEL} ${FAST5_ACTIVE_DIR} > ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.fst.sam
# dorado basecaller --verbose --batchsize 0 --reference ${MAPPING_REFERENCE} -k 15 -w 10 -g 16 --batch-timeout 100000 --num-runners 2 --emit-sam ${DORADO_FST_MODEL} ${FAST5_CONTROL_DIR} > ${ANALYSIS_DIR}/${ANALYSIS_ID}.control.fst.sam

# Gather the unblocked read identifiers
python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py endreason --fast5 ${FAST5_ACTIVE_DIR} --output ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.endreasons.csv
# python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py endreason --fast5 ${FAST5_CONTROL_DIR} --output ${ANALYSIS_DIR}/${ANALYSIS_ID}.control.endreasons.csv

python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py evaluation \
    --summary-table ${ANALYSIS_DIR}/${ANALYSIS_ID}.summary.csv \
    --active-sam ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.fst.sam \
    --active-ends ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.endreasons.csv \
    --active-output ${ANALYSIS_DIR}/${ANALYSIS_ID}.active.summary.csv # \
    # --control-sam ${ANALYSIS_DIR}/${ANALYSIS_ID}.control.fst.sam \
    # --control-ends ${ANALYSIS_DIR}/${ANALYSIS_ID}.control.endreasons.csv \
    # --control-output ${ANALYSIS_DIR}/${ANALYSIS_ID}.control.summary.csv


