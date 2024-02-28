
process StreamfishSimulation {

    label "streamfish_sim"
    
    publishDir "$params.outdir/$dataset_name/$config_name/$output_subdir/$replicate", mode: "symlink", pattern: "sim/*.blow5"

    input:
    val control
    each replicate
    each config
    each dataset

    output:
    tuple val(replicate), val(config_name), val(dataset_name), file("sim")
    tuple val(replicate), val(config_name), val(dataset_name), file("sim/*.blow5")

    script:

    dataset_name = dataset.getBaseName()
    config_name = config.getBaseName()
    output_subdir = control ? "control" : "experiment"

    if (control) {
        """
        streamfish read-until --config $config --simulation $dataset --outdir sim/ --control
        """
    } else {
        """
        streamfish read-until --config $config --simulation $dataset --outdir sim/
        """
    }

}


process StreamfishEvaluation {

    label "streamfish_eval"

    input:
    tuple val(replicate), val(config_name), val(dataset_name), file(sim_dir)

    output:
    

    script:
    config_name = config.baseName()

    """
    streamfish evaluate cipher --directory $sim_dir --reads 
    """

}


process CipherSimulateCommunity {

    label "cipher_sim"

    input:
    each config

    output:
    tuple val(config_name), file("$config_name/"), file("$config_name/${params.prefix}_community.blow5"), file("$config_name/${params.prefix}_community.signal.reads.tsv")

    script:
    config_name = config.baseName()

    """
    cipher simulate-community --config $config --outdir $config_name --prefix $params.prefix --overflow 0.1 --threads $task.cpus
    """

}
