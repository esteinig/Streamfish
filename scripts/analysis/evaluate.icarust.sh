#!/usr/env/bash

# =============================
# ICARUST EXPERIMENT EVALUATION
# =============================

# Run on host - uses containers to execute basecalling and other specific tasks. The following steps
# require the container network to be configured and executed in development mode, for example:

# docker compose -f docker/docker-compose.yml --profile dev --project-name main --env-file docker/.env up

# Enter the evaluation container and setup the conda environment if not already done:

# micromamba create -f scripts/analysis/evaluate.icarust.yml -y

# =============================
# STEP 1: DORADO BASECALLING
# =============================

ANALYSIS_ID='test'
ANALYSIS_DIR="/tmp/streamfish_analysis_${ANALYSIS_ID}"

MAPPING_REFERENCE='/tmp/bacteria.mmi'
FAST5_ACTIVE_DIR="/tmp/experiment_1/active"
FAST5_CONTROL_DIR="/tmp/experiment_1/control"

DORADO_FST_MODEL='/tmp/models/dna_r9.4.1_e8_fast@v3.4'

# DORADO_HAC_MODEL='/tmp/models/dna_r9.4.1_e8_hac@v3.3'
# DORADO_SUP_MODEL='/tmp/models/dna_r9.4.1_e8_sup@v3.3'

micromamba activate icarust_evaluation

# Run the Dorado basecaller with FAST, HAC and SUP models on the Fast5 outputs from Icarust; use the same 
# aligner and settings as implemented in Streamfish [test version with Dorado]. Batchsize is automatically
# chosen and batch timeout is reset to the default setting.
dorado basecaller --verbose --batchsize 0 --reference ${MAPPING_REFERENCE} -k 15 -w 10 -g 16 --batch-timeout 100 --num-runners 2 --emit-sam ${DORADO_FST_MODEL} ${FAST5_OUTPUT_DIR} > ${ANALYSIS_DIR}/${ANALYSIS_ID}.fst.sam

# Gather the unblocked read identifiers
python /usr/src/streamfish/scripts/analysis/evaluate.icarust.py endreason-fast5 --fast5 ${FAST5_OUTPUT_DIR} --output ${ANALYSIS_DIR}/${ANALYSIS_ID}.endreasons.csv"


