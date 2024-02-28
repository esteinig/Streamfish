/*

Nextflow pipeline for multiple GPU deployment of experiment runs with Streamfish

TODO: ensure that latency is not compromised by the Nextflow runner

**/

params.outdir = "test/"
params.threads = 16
params.control = true
params.replicates = 3
params.simulations = "*.blow5"
params.configs = "*.toml"

include { StreamfishSimulation as StreamfishSimulationExperiment } from "./nextflow/process.nf";
include { StreamfishSimulation as StreamfishSimulationControl } from "./nextflow/process.nf";

workflow {

    config_files = Channel.fromPath(params.configs) | view
    simulation_files = Channel.fromPath(params.simulations) | view

    StreamfishSimulationExperiment(false, 1..params.replicates, config_files, simulation_files)

    if (params.control) {
        StreamfishSimulationControl(true, 1..params.replicates, config_files, simulation_files)
    }
    
    
}