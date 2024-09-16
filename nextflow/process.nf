
process StreamfishSimulation {

    label "streamfish_sim"
    
    publishDir "$params.outdir/blow5", mode: "symlink", pattern: "${identifier}"
    publishDir "$params.outdir/evals", mode: "copy", pattern: "${identifier}.tsv"
    publishDir "$params.outdir/sims/${identifier}", mode: "symlink", pattern: "${blow5}"
    publishDir "$params.outdir/sims/${identifier}", mode: "symlink", pattern: "${toml}"
    publishDir "$params.outdir/sims/${identifier}", mode: "symlink", pattern: "${tsv}"
    publishDir "$params.outdir/sims/${identifier}", mode: "symlink", pattern: "${exp}"
    
    input:
    tuple file(blow5), file(toml), file(exp), file(tsv)
    each replicate
    each model
    val control

    output:
    tuple val(replicate), val(config_name), val(simulation_name), val(ctrl_exp), file("${identifier}"), file("${identifier}.tsv")
    tuple file(blow5), file(toml), file(exp), file(tsv)
    script:

    ctrl_exp = control ? "control" : "experiment"

    simulation_name = blow5.getBaseName()
    config_name = toml.getBaseName()
    exp_name = exp.getBaseName()

    identifier = "${simulation_name}__${config_name}__${model}__${exp_name}__${ctrl_exp}__${replicate}"

    if (control) {
        """
        streamfish read-until --config $toml --simulation $blow5 --outdir $identifier/ --basecaller-server $params.basecaller_server --model $model --experiment $exp --control 
        streamfish evaluate cipher --directory $identifier/ --metadata $tsv --output ${identifier}.tsv
        """
    } else {
        """
        streamfish read-until --config $toml --simulation $blow5 --outdir $identifier/ --basecaller-server $params.basecaller_server --model $model --experiment $exp
        streamfish evaluate cipher --directory $identifier/ --metadata $tsv --output ${identifier}.tsv
        """
    }

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
