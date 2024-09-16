# Experiment 2: Simple host-pathogen bacterial or viral low abundance infection mock sample simulations

Simulated mock samples for this experiment series:

1. RNA virus: SARS-CoV-2 in human background (CHM13v2 T2T reference) - unrealistic with some read lengths and assuming DNA library for test purposes and moderate genome size (30kb) - baseline host-viral system: `sars/test_6.toml` 10 kbp mean read length, `dna-r10-min` 5Khz (Cipher) database holdout experiment: CHM13v2 with (1) Wuhan-1 simulated strain, (2) Virosaurus98 with Wuhan-1 reference (3) Virosaurus98 with all SARS-CoV-2 references removed, `dorado`, `guppy` with `fast`, `hac` [TBD: 2.5 kbp, 5 kbp, 10 kbp, 20 kbp for full 24 hour runs]. 3 replicates and control for each, 30 mins runtime.

```terminal
nextflow run main.nf
```

2. RNA virus: JEV in human background (CHM13v2 T2T reference) - unrealistic with some read lengths and assuming DNA library for test purposes and moderate genome size (30kb) - baseline host-viral system: `jev/test_6.toml` 10 kbp mean read length, `dna-r10-min` 5Khz (Cipher) database holdout experiment: CHM13v2 with (1) Wuhan-1 simulated strain, (2) Virosaurus98 with Wuhan-1 reference (3) Virosaurus98 with all SARS-CoV-2 references removed, `dorado`, `guppy` with `fast`, `hac` [TBD: 2.5 kbp, 5 kbp, 10 kbp, 20 kbp for full 24 hour runs]. 3 replicates and control for each, 30 mins runtime.

```terminal
nextflow run main.nf
```

3. DNA virus: Human alphaherpesvirus 3 (VZV) DNA virus in host background (CHM13v2 T2T reference) - realistic host-viral library that could be sequenced

```terminal
nextflow run main.nf
```

4. DNA virus: Monkeypox virus (Mpox) DNA virus in host background (CHM13v2 T2T reference) - realistic host-viral library that could be sequenced

```terminal
nextflow run main.nf
```

5. Staphylococcus aureus two-strain system - community MRSA ST93 and ST772 (allows for later taxonomic systems with increasing distance from outbreak level)


```terminal
nextflow run main.nf
```

6. Mycobacterium sp. nov. SOLO unknown mycobacterial species with another bacterium e.g. Staphylococcus aureus - baseline bacterial system\

```terminal
nextflow run main.nf
```


Con