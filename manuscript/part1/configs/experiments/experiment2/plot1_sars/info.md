# Experiment 2: Simple two-member bacterial or viral infection mock samples

Simulated mock samples for this experiment series

1. SARS-CoV-2 in human background (CHM13v2 T2T reference) - unrealistic with some read lengths and assuming DNA library for test purposes and moderate genome size (30kb) - baseline host-viral system: `sars/test_6.toml` 10 kbp mean read length, `dna-r10-min` 5Khz (Cipher) database holdout experiment: CHM13v2 with (1) Wuhan-1 simulated strain, (2) Virosaurus98 with Wuhan-1 reference (3) Virosaurus98 with all SARS-CoV-2 references removed, `dorado`, `guppy` with `fast`, `hac` [TBD: 2.5 kbp, 5 kbp, 10 kbp, 20 kbp for full 24 hour runs]. 3 replicates and control for each, 30 mins runtime.

```terminal
nextflow run main.nf --outdir test_holdout_sars_db --simulations cipher/test_sars_6/sars_host_10k.blow5 --configs ""
```

2. Staphylococcus aureus two-strain taxonomic systems with increasing distance from outbreak level up to phyla
3. Mycobacterium sp. nov. SOLO unknown mycobacterial species with another bacterium i.e. Staphylococcus aureus - baseline bacterial system
4. Human alphaherpesvirus 3 (VZV) DNA virus in host background (CHM13v2 T2T reference) - realistic host-viral library that could be sequenced

Controls 