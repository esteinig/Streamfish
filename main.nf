/*

Nextflow pipeline for multiple GPU deployment of experiment runs with Streamfish

TODO: ensure that latency is not compromised by the Nextflow runner

**/

params.outdir = "test/"
params.configs = "streamfish/*.toml"
params.simulations = "cipher/*.blow5"
params.metadata = "cipher/*.tsv"

params.controls = false
params.replicates = 3

params.basecaller_server = "/opt/ont/guppy/bin/guppy_basecall_server"
// /opt/ont/ont-dorado-server/bin/dorado_basecall_server

params.models = [
    "dna_r10.4.1_e8.2_400bps_5khz_hac", 
] 
// "dna_r10.4.1_e8.2_400bps_5khz_sup"
// "dna_r10.4.1_e8.2_400bps_5khz_fast", 

params.experiments = "experiment/*.exp.toml"

include { StreamfishSimulation as StreamfishSimulationExperiment } from "./nextflow/process.nf";
include { StreamfishSimulation as StreamfishSimulationControl } from "./nextflow/process.nf";

workflow {

    simulation_data = matchSimulationFiles(
        Channel.fromPath(params.simulations),
        Channel.fromPath(params.metadata),
        Channel.fromPath(params.configs),
        Channel.fromPath(params.experiments)
    ) | view
    
    StreamfishSimulationExperiment(simulation_data, 1..params.replicates, params.models, false)
    if (params.controls) StreamfishSimulationControl(simulation_data, 1..params.replicates, params.models, true)
    
}

// Function to match Paths based on shared names after removing specified suffixes
def matchSimulationFiles(simulations, metadata, configs, experiments) {

    def normalizeName = { path ->
        path.getFileName().toString().replaceAll(".blow5", "").replaceAll(".signal.reads.tsv", "")
    }

    // Create maps to store normalized names and their corresponding Paths
    def map1 = simulations.collect { tuple(normalizeName(it), it) }
    def map2 = metadata.collect { tuple(normalizeName(it), it) }

    // Sorts combined/crossed-over data by file name extension: blow5, toml, tsv
    return map1.mix(map2).groupTuple().map { tuple(it[1][1], it[1][0]) }.combine(configs.combine(experiments)).map { simdata ->
        data = simdata.sort { a, b -> 
            a.getFileName().toString().tokenize('.').last() <=> b.getFileName().toString().tokenize('.').last() 
        }
    }
}