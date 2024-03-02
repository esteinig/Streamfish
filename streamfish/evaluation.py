from dataclasses import dataclass
from pathlib import Path
from matplotlib import pyplot as plt
from matplotlib.ticker import FuncFormatter


from typing import List

import numpy as np
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

def plot_relative_signal(
    experiments: List[Path], 
    controls: List[Path],
    sim_tags: List[str] = [],
    interval: int = 10,
    prefix: str = "", 
    outdir: Path = Path.cwd(), 
    title: str = "Relative cumlative signal of simulated microbiome",
    plot_size: str = "24,12",
    plot_format: str = "pdf",
    by_chromosome: bool = False,
    diff_limit: float = 300
):
    sns.set_theme(style="darkgrid")

    print(experiments+controls)
    
    timeseries = get_signal_time_series(data=experiments+controls, sim_tags=sim_tags, by_chromosome=by_chromosome, interval=interval)    
    diff = get_experiment_control_diff(timeseries=timeseries)

    diff = diff[diff["diff"] != 0]

    print(timeseries)
    print(diff)

    palette = sns.color_palette(
        "mako_r", len(timeseries.member.unique())
    )

    fig1, axes = plt.subplots(
        nrows=1, ncols=3, figsize=[int(s.strip()) for s in plot_size.split(",")]
    )

    axes = axes.flat

    p1 = sns.lineplot(data=timeseries, x="second", y="relative_signal", hue="member", style="experiment", ax=axes[0], linewidth=2.5, palette=palette)
    p2 = sns.lineplot(data=timeseries, x="second", y="total_signal", hue="member", style="experiment", ax=axes[1], linewidth=2.5, palette=palette)
    

    p3 = sns.lineplot(data=diff, x="second", y="diff", hue="member", ax=axes[2], linewidth=2.5, palette=palette)
    
    p3.set_ylim(0, max(diff['diff']) if diff_limit <= 0 else diff_limit)
    p3.set_yticks(np.arange(start=0, stop=max(diff['diff']) if diff_limit <= 0 else diff_limit, step=20))
    p3.yaxis.set_major_formatter(FuncFormatter(lambda y, _: f'{y:.0f}%'))
    
    p1.set_xlabel("\nSeconds")
    p1.set_ylabel("Relative cumulative signal\n")
    p1.set_ylim(0)

    p2.set_xlabel("\nSeconds")
    p2.set_ylabel("Total cumulative signal\n")


    p3.set_xlabel("\nSeconds")
    p3.set_ylabel("Percent difference in total culumulative signal (experiment vs. control)\n")

    plt.suptitle(title, fontsize=18)
    

    prefix = prefix+"." if prefix else ""
    fig1.savefig(outdir / f"{prefix}signal_timeseries.{plot_format}", dpi=300, transparent=False)


def get_experiment_control_diff(timeseries: pandas.DataFrame):

    diff_data = []
    for sec, data in timeseries.groupby("second"):
        for member, member_data in data.groupby("member"):
            for rep, replicate_data in member_data.groupby("replicate"):

                control_data = replicate_data[replicate_data["experiment"] == "control"]
                experiment_data = replicate_data[replicate_data["experiment"] == "experiment"]

                if len(control_data) > 1:
                    print("CONTROL DATA > 1")
                if len(experiment_data) > 1:
                    print("EXPERIMENT DATA > 1")

                if control_data.empty and not experiment_data.empty:
                    print(f"No control data for member {member} at time {sec}")
                    diff_data.append([sec, member, rep, 0])
                elif experiment_data.empty and not control_data.empty:
                    print(f"No experiment data for member {member} at time {sec}")
                    diff_data.append([sec, member, rep, 0])
                elif experiment_data.empty and control_data.empty:
                    continue
                else:
                    diff_exp_ctrl = float((experiment_data.at[experiment_data.index[0], "total_signal"] / control_data.at[control_data.index[0], "total_signal"])*100)
                    diff_data.append([sec, member, rep, diff_exp_ctrl])

    return pandas.DataFrame(diff_data, columns=["second", "member", "replicate", "diff"])

def get_signal_time_series(data: List[Path], sim_tags: List[str], by_chromosome: bool, interval: int) -> pandas.DataFrame:
    
    timeseries_data = []
    for i, path in enumerate(data):
        df = pandas.read_csv(path, sep="\t", header=0)

        configs = path.stem.split("__")
        cfg, exp_ctrl, rep = None, None, None

        # Only if we have compliant file name from pipeline
        if len(configs) == 4:
            sim, cfg, exp_ctrl, rep = configs
        else:
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
                    try:
                        relative_cumulative_signal = ref_dict[member] / ref_signal
                    except ZeroDivisionError:
                        relative_cumulative_signal = 0
                    
                    print(f"Second: {second}, Member: {member}, Total: {total_cumulative_signal} Relative: {relative_cumulative_signal}")

                    timeseries_data.append([sim, cfg, exp_ctrl, rep, second, member, total_cumulative_signal, relative_cumulative_signal])

    return pandas.DataFrame(timeseries_data, columns=["simulation", "config", "experiment", "replicate", "second", "member", "total_signal", "relative_signal"])