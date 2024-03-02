/*

Nextflow pipeline for multiple GPU deployment of experiment runs with Streamfish

TODO: ensure that latency is not compromised by the Nextflow runner

**/

params.outdir = "test/"
params.configs = "*.toml"
params.simulations = "*.blow5"
params.metadata = "*.tsv"

params.controls = false
params.replicates = 3

include { StreamfishSimulation as StreamfishSimulationExperiment } from "./nextflow/process.nf";
include { StreamfishSimulation as StreamfishSimulationControl } from "./nextflow/process.nf";

workflow {

    simulation_data = matchSimulationFiles(
        Channel.fromPath(params.simulations),
        Channel.fromPath(params.metadata),
        Channel.fromPath(params.configs)
    )
    
    StreamfishSimulationExperiment(simulation_data, 1..params.replicates, false)
    if (params.controls) StreamfishSimulationControl(simulation_data, 1..params.replicates, true)


    
    
}

// Function to match Paths based on shared names after removing specified suffixes
def matchSimulationFiles(simulations, metadata, configs) {

    def normalizeName = { path ->
        path.getFileName().toString().replaceAll(".blow5", "").replaceAll(".signal.reads.tsv", "")
    }

    // Create maps to store normalized names and their corresponding Paths
    def map1 = simulations.collect { tuple(normalizeName(it), it) }
    def map2 = metadata.collect { tuple(normalizeName(it), it) }

    // Sorts combined/crossed-over data by file name extension: blow5, toml, tsv
    return map1.mix(map2).groupTuple().map { tuple(it[1][1], it[1][0]) }.combine(configs).map { simdata ->
        data = simdata.sort { a, b -> 
            a.getFileName().toString().tokenize('.').last() <=> b.getFileName().toString().tokenize('.').last() 
        }
    }
}