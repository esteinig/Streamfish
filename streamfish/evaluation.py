from dataclasses import dataclass
from pathlib import Path
from matplotlib import pyplot as plt
from typing import List

import seaborn as sns
import pandas

@dataclass
class CipherTimeSeriesEvaluation:
    uuid: str
    start_time_seconds: float
    end_time_seconds: float
    read_sequenced_seconds: float
    read_completed: bool
    output_signal_length: int
    simulated_signal_length: int
    channel_number: str
    read_number: int
    end_reason: str
    ref_id: str
    ref_start: int
    ref_end: int
    ref_strand: str
    read_length: int
    member_id: str
    member_role: str
    member_target: bool
    member_abundance: float
    member_taxid: str
    member_scientific_name: str
    member_accession: str

def plot_relative_frequency(
    paths: List[Path], 
    sim_tags: List[str] = [],
    interval: int = 10,
    prefix: str = "", 
    outdir: Path = Path.cwd(), 
    title: str = "Relative cumlative signal of simulated microbiome",
    plot_size: str = "18,12",
    plot_format: str = "pdf",
    by_chromosome: bool = False
):
    sns.set_theme(style="darkgrid")
    

    data = []
    for i, path in enumerate(paths):
        df = pandas.read_csv(path, sep="\t", header=0)

        try: 
            sim = sim_tags[i]
        except IndexError:
            sim = str(i)
        
        completed = int(max(df.end_time_seconds))

        if not by_chromosome:
            df["ref_id"] = ["Host" if ref_id.startswith("chr") else ref_id for ref_id in df["ref_id"]]

        ref_signal = 0
        ref_dict = {k: 0 for k in df.ref_id.unique()}
        # For each second in the sequencing run...
        for second in range(1, completed + 1, interval):
            second_data = df.loc[(df.end_time_seconds >= second) & (df.end_time_seconds < second + 1)]
            if not second_data.empty:

                ref_signal += sum(second_data["output_signal_length"])

                # Cumulative signal sum for each member and total
                for member, group in second_data.groupby('ref_id'):
                    ref_dict[member] += sum(group['output_signal_length'])

                    total_cumulative_signal = ref_dict[member]
                    
                    # Calculate the relative contribution of each signal length
                    relative_cumulative_signal =  ref_dict[member] / ref_signal
                    
                    print(f"Second: {second}, Member: {member}, Total: {total_cumulative_signal} Relative: {relative_cumulative_signal}")

                    data.append([second, f"{member}-{sim}", total_cumulative_signal, relative_cumulative_signal])
    
    signal_time_series = pandas.DataFrame(data, columns=["second", "member", "total_signal", "relative_signal"])
    
    fig1, axes = plt.subplots(nrows=1, ncols=2, figsize=[int(s.strip()) for s in plot_size.split(",")])
    
    axes = axes.flat

    p1 = sns.lineplot(data=signal_time_series, x="second", y="relative_signal",hue="member", ax=axes[0], linewidth=2.5)
    p2 = sns.lineplot(data=signal_time_series, x="second", y="total_signal", hue="member", ax=axes[1], linewidth=2.5)

    p1.set_xlabel("\nSeconds")
    p1.set_ylabel("Relative cumulative signal\n")

    p2.set_xlabel("\nSeconds")
    p2.set_ylabel("Total cumulative signal\n")

    plt.suptitle(title, fontsize=18)

    prefix = prefix+"." if prefix else ""

    fig1.savefig(outdir / f"{prefix}signal_timeseries.{plot_format}", dpi=300, transparent=False)
