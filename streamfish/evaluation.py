from dataclasses import dataclass
from pathlib import Path
from matplotlib import pyplot as plt
from matplotlib.ticker import FuncFormatter
import matplotlib.lines as mlines

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

def plot_signal_distribution(
    summaries: List[Path], 
    variants: List[str] | None = None,
    sim_tags: List[str] = [],
    interval: int = 10,
    prefix: str = "", 
    outdir: Path = Path.cwd(), 
    title: str = "Relative cumlative signal of simulated microbiome",
    plot_size: str = "24,12",
    plot_format: str = "pdf",
    by_chromosome: bool = False,
    diff_limit: float = 300,
):
    sns.set_theme(style="darkgrid")
    prefix = prefix+"." if prefix else ""
    
    data = get_signal_distribution_data(
        data=summaries, 
        sim_tags=sim_tags, 
        by_chromosome=by_chromosome, 
        interval=interval,
        color_variants=variants
    )    

    num_plots = 1
    num_configs = 3
    fig1, axes = plt.subplots(
        nrows=num_configs, ncols=num_plots, figsize=(num_plots*12, num_configs*12)
    )

    axes = axes.flat
    
    i = 0
    for config, cfg_data in data.groupby("config"):
        p = sns.histplot(data=cfg_data, x="output_signal_length", hue="experiment", kde=True, fill=True, alpha=0.5, ax=axes[i])
        p.set_title(config)
        i += 1

    fig1.savefig(outdir / f"{prefix}_dist.{plot_format}", dpi=300, transparent=False)
    data.to_csv(outdir / f"{prefix}_full.tsv", sep="\t", index=False)
    


def plot_relative_signal(
    summaries: List[Path], 
    variants: List[str] | None = None,
    sim_tags: List[str] = [],
    interval: int = 10,
    prefix: str = "", 
    outdir: Path = Path.cwd(), 
    title: str = "Relative cumlative signal of simulated microbiome",
    plot_size: str = "24,12",
    plot_format: str = "pdf",
    by_chromosome: bool = False,
    diff_limit: float = 300,
):
    sns.set_theme(style="darkgrid")
    prefix = prefix+"." if prefix else ""
    
    timeseries = get_signal_time_series(
        data=summaries, 
        sim_tags=sim_tags, 
        by_chromosome=by_chromosome, 
        interval=interval,
        color_variants=variants
    )    
    diff = get_experiment_control_diff(timeseries=timeseries)

    timeseries.to_csv(outdir / f"{prefix}signal_timeseries.tsv", sep="\t", index=False)
    diff.to_csv(outdir / f"{prefix}signal_timeseries.diff_zero.tsv", sep="\t", index=False)

    diff = diff[diff["diff"] != 0]  # needed?

    print(diff)

    diff.to_csv(outdir / f"{prefix}signal_timeseries.diff.tsv", sep="\t", index=False)

    configs = sorted(timeseries.config.unique().tolist())
    num_configs = len(configs)

    members = sorted(timeseries.member.unique().tolist())
    num_members = len(members)
    
    num_plots = 3
    fig1, axes = plt.subplots(
        nrows=num_configs, ncols=num_plots, figsize=(num_plots*12, num_configs*12)
    )
    
    palettes = [
        ["#1A3D82", "#4499F5"], ["#415521", "#97AD3D"], ["#F9B40E", "#FDE16A"],  # blue, green, purple
    ]
    if variants:
        variants = sorted(timeseries["variant"].unique())
        palettes = [sns.color_palette(pal, n_colors=num_members) for pal in palettes[:len(variants)]]
    else: 
        variants = [None]
        palettes = [sns.color_palette("mako_r", n_colors=num_members)]
    
    num_variants = len(variants)

    # Custom legend handles
    exp_line = mlines.Line2D([], [], color='black', linestyle='-', lw=2, label='Experiment')
    ctrl_line = mlines.Line2D([], [], color='black', linestyle='--', lw=2, label='Control')

    # Member variant handles
    member_variant_lines = [
        mlines.Line2D([], [], color=palettes[i][j], linestyle='-', lw=2, label=f"{member} ({variant})") for i, variant in enumerate(variants) for j, member in enumerate(members)
    ]

    # Rows: configs, columns: plots
    for n_cfg, cfg in enumerate(configs):

        ts = timeseries[timeseries["config"] == cfg]
        df = diff[diff["config"] == cfg]

        print(ts)
        print(df)

        print(f"Plotting configuration: `{cfg}` ({n_cfg})")
        for i, variant in enumerate(variants):
            print(f"Variant {variant}")

            palette = palettes[i]

            if variant is not None:
                tsvar = ts[ts["variant"] == variant]
                dfvar = df[df["variant"] == variant]
            else:
                tsvar = ts 
                dfvar = df

            
            p1 = sns.lineplot(
                data=tsvar, x="second", y="relative_signal", hue="member", style="experiment", 
                ax=axes[n_cfg][0], linewidth=2.5, palette=palette, legend=None, style_order=["experiment", "control"], hue_order=members
            )
            p2 = sns.lineplot(
                data=tsvar, x="second", y="total_signal", hue="member", style="experiment", 
                ax=axes[n_cfg][1], linewidth=2.5, palette=palette, legend=None, style_order=["experiment", "control"], hue_order=members
            )
            p3 = sns.lineplot(data=dfvar, x="second", y="diff", hue="member", ax=axes[n_cfg][2], linewidth=2.5, palette=palette, legend=None)
            
        p3.set_ylim(0, max(df['diff']) if diff_limit <= 0 else diff_limit)
        p3.set_yticks(np.arange(start=0, stop=max(df['diff']) if diff_limit <= 0 else diff_limit, step=50))
        p3.yaxis.set_major_formatter(FuncFormatter(lambda y, _: f'{y:.0f}%'))
        
        p1.set_xlabel("\nSeconds")
        p1.set_ylabel("Relative cumulative signal\n")
        p1.set_ylim(0)

        p2.set_xlabel("\nSeconds")
        p2.set_ylabel("Total cumulative signal\n")

        p3.set_xlabel("\nSeconds")
        p3.set_ylabel("Percent of total culumulative signal (experiment vs. control)\n")

        # Title on first plot of row with config description
        p1.set_title(label=cfg, fontsize=18, fontweight="bold")
        
        # Bottom middle legend
        if n_cfg == num_configs-1:
            p2.legend(
                handles=[exp_line, ctrl_line]+member_variant_lines, 
                loc='upper center', 
                bbox_to_anchor=(0.5, -0.15), 
                fancybox=True, 
                shadow=True,
                ncol=num_variants+1, 
                fontsize=18,
                handlelength=3, 
                handletextpad=1, 
                markerscale=2,
            )

    plt.suptitle(title, fontsize=24, fontweight="bold")

    fig1.savefig(outdir / f"{prefix}signal_timeseries.{plot_format}", dpi=300, transparent=False)


def get_experiment_control_diff(timeseries: pandas.DataFrame):

    diff_data = []
    for sim, sim_data in timeseries.groupby("simulation"):
        for cfg, cfg_data in sim_data.groupby("config"):
            for var, variant_data in cfg_data.groupby("variant", dropna=False):
                for sec, data in variant_data.groupby("second"):
                    for member, member_data in data.groupby("member"):
                        for rep, replicate_data in member_data.groupby("replicate"):
                                
                            control_data = replicate_data[replicate_data["experiment"] == "control"]
                            experiment_data = replicate_data[replicate_data["experiment"] == "experiment"]

                            if len(control_data) > 1:
                                raise ValueError
                            if len(experiment_data) > 1:
                                raise ValueError

                            if control_data.empty and not experiment_data.empty:

                                # Setting to 0 and removing 0 entries from dataframe results in skipping that datapoint and connecting the lineplot to
                                # the next data point which is correct as well - however it may be better to use the previous time-slice difference value 
                                # of the same member-replicate, which simply makes the lineplot at these points step-wise, which should better represent
                                # that in this time-slice, the summary of signal values simply did not include control or experiment data for the specific 
                                # member in this replicate (occurs more often e.g. in abundant host background simulations where the target member may not 
                                # have been sequenced in this replicate for several seconds under this experiment/control condition)

                                print(f"No control data for member {member} at time {sec} - using zero value for filtering") 
                                diff_data.append([sim, cfg, var, sec, member, rep, 0])
                            elif experiment_data.empty and not control_data.empty:
                                print(f"No experiment data for member {member} at time {sec} - using zero value for filtering")
                                diff_data.append([sim, cfg, var, sec, member, rep, 0])
                            elif experiment_data.empty and control_data.empty:
                                continue
                            else:
                                diff_exp_ctrl = float((experiment_data.at[experiment_data.index[0], "total_signal"] / control_data.at[control_data.index[0], "total_signal"])*100)
                                diff_data.append([sim, cfg, var, sec, member, rep, diff_exp_ctrl])

    return pandas.DataFrame(diff_data, columns=["simulation", "config", "variant", "second", "member", "replicate", "diff"])

def get_signal_time_series(data: List[Path], sim_tags: List[str], by_chromosome: bool, interval: int, color_variants: List[str] | None = None) -> pandas.DataFrame:
    
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
                    
                    if color_variants and cfg:
                        variant_present = [v for v in color_variants if v in cfg]  # variants are substrings of the config name
                        variant = variant_present[0] if variant_present else None
                        var_cfg = cfg.replace(f"{variant}", "").replace("__", "_").strip("_")
                    else:
                        variant = None 
                        var_cfg = cfg

                    print(f"Second: {second}, Member: {member}, Total: {total_cumulative_signal} Relative: {relative_cumulative_signal}")

                    timeseries_data.append([sim, var_cfg, variant, exp_ctrl, rep, second, member, total_cumulative_signal, relative_cumulative_signal])

    return pandas.DataFrame(timeseries_data, columns=["simulation", "config", "variant", "experiment", "replicate", "second", "member", "total_signal", "relative_signal"])

def get_signal_distribution_data(data: List[Path], sim_tags: List[str], by_chromosome: bool, interval: int, color_variants: List[str] | None = None) -> pandas.DataFrame:


    distribution_data = []
    for i, path in enumerate(data):
        
        print(f"Processing file: {path}")

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
        

        if not by_chromosome:
            df["ref_id"] = ["Host" if ref_id.startswith("chr") else ref_id for ref_id in df["ref_id"]]
        

        if color_variants and cfg:
            variant_present = [v for v in color_variants if v in cfg]  # variants are substrings of the config name
            variant = variant_present[0] if variant_present else None
            var_cfg = cfg.replace(f"{variant}", "").replace("__", "_").strip("_")
        else:
            variant = None 
            var_cfg = cfg

        df["simulation"] = [sim for _ in df.iterrows()]
        df["config"] = [var_cfg for _ in df.iterrows()]
        df["variant"] = [variant for _ in df.iterrows()]
        df["experiment"] = [exp_ctrl for _ in df.iterrows()]
        df["replicate"] = [rep for _ in df.iterrows()]

        distribution_data.append(df[["simulation", "config", "variant", "experiment", "replicate", "ref_id", "output_signal_length"]])

    return pandas.concat(distribution_data)
