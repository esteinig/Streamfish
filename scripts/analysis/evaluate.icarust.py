"""
Extracts the Icarust v0.3.0 (53ca111a3b20563a2c9ca22bba766aef2d95deb5) endreasons from Fast5

0 = Default
1 = Positive Signal
4 = Unblock

pip install typer ont-fast5-api seaborn matplotlib

"""

import typer
import statistics

import seaborn as sns
import matplotlib.pyplot as plt

from dataclasses import dataclass
from typing import List, Dict
from pprint import pprint

from pathlib import Path
from ont_fast5_api.fast5_interface import get_fast5_file


LAPUTA_MEDIUM = [
    '#14191F',
    '#1D2645',
    '#403369',
    '#5C5992',
    '#AE93BE',
    '#B4DAE5',
    '#F0D77B',
]

YESTERDAY_MEDIUM = [
    '#061A21',
    '#132E41',
    '#26432F',
    '#4D6D93',
    '#6FB382',
    '#DCCA2C',
    '#92BBD9',
]

LAPUTA_MEDIUM.reverse()
YESTERDAY_MEDIUM.reverse()

@dataclass
class MappingSummary:

    read_lengths: List[int]

    reads: int = 0
    bases: int = 0
    unblocked: int = 0
    unblocked_percent: float = 0.
    n50_length: int = 0
    mean_length: float = 0.
    median_length: float = 0.


def compute_n50(read_lengths):
    # Sort the read lengths in descending order
    sorted_lengths = sorted(read_lengths, reverse=True)

    # Calculate the total sum of read lengths
    total_length = sum(sorted_lengths)

    # Calculate the threshold value for N50
    threshold = total_length / 2

    # Iterate over the sorted lengths and find the N50 length
    running_sum = 0
    n50 = None
    for length in sorted_lengths:
        running_sum += length
        if running_sum >= threshold:
            n50 = length
            break

    return n50

def read_length_density_all(data: Dict[str, MappingSummary], output_file: str, min_length: int =50, max_length: int =4000, colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.6)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, ax = plt.subplots()
    
    for i, (ref, summary) in enumerate(data.items()): 
        lengths = [d for d in summary.read_lengths if d >= min_length and d < max_length]
        sns.kdeplot(lengths, ax=ax, label=ref, color=colors[i+1], fill=True, alpha=0.8)

    # Set the plot labels and title
    ax.set_xlabel('bp')
    ax.set_ylabel('Density')
    ax.set_title('Read length', fontdict={'weight': 'bold'})
    ax.legend()

    sns.despine()
    ax.grid(False)
    
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()

def read_length_histogram_all(data: Dict[str, MappingSummary], output_file: str, min_length: int =50, max_length: int =4000, colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.6)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, ax = plt.subplots()
    
    for i, (ref, summary) in enumerate(data.items()): 
        lengths = [d for d in summary.read_lengths if d >= min_length and d < max_length]
        sns.histplot(lengths, kde=False, ax=ax, color=colors[i+1], bins='auto', fill=True, alpha=0.8)

    # Set the plot labels and title
    ax.set_xlabel('bp')
    ax.set_ylabel('Reads')
    ax.set_title('Read length', fontdict={'weight': 'bold'})

    sns.despine()
    ax.grid(False)
    
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()

def read_length_histogram_distinct(data: Dict[str, MappingSummary], output_file: str, min_length: int =50, max_lengths: List[int] = [4000], colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.4)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, axes = plt.subplots(nrows=1, ncols=len(data))
    
    for i, (ref, summary) in enumerate(data.items()): 
        ax = axes[i]

        try:
            max_length = max_lengths[i]
        except IndexError:
            max_length = max_lengths[0]

        lengths = [d for d in summary.read_lengths if d >= min_length and d < max_length]
        sns.histplot(lengths, kde=False, ax=ax, color=colors[i+1], bins='auto', fill=True, alpha=1.)

        # Set the plot labels and title
        ax.set_xlabel('bp')
        ax.set_ylabel('Reads')
        ax.set_title(ref, fontdict={'weight': 'bold'})

        sns.despine()
        ax.grid(False)
    
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()


app = typer.Typer(add_completion=False)


@app.command()
def endreason_fast5(
    fast5: Path = typer.Option(
        ..., help="Directory of Fast5 files from Icarust to extract end-reason"
    ),
    output: Path = typer.Option(
        ..., help="Output table in CSV"
    )
):
    """
    Get a table of read identifiers and end reasons from Fast5 (Icarust v0.3.0)
    """

    out_handle = output.open("w")
    out_handle.write("id,channel,number,endreason\n")

    for file in fast5.glob("*.fast5"):
        with get_fast5_file(file, mode="r") as f5:
            for read in f5.get_reads():
                channel_attrs = dict(read.handle[read.global_key + 'channel_id'].attrs)
                raw_attrs = dict(read.handle[read.global_key + 'Raw'].attrs)

                read_id = read.read_id
                read_channel = channel_attrs.get("channel_number")
                read_number = raw_attrs.get("read_number")
                read_endreason = raw_attrs.get("end_reason")

                out_handle.write(f"{read_id},{read_channel},{read_number},{read_endreason}\n")

    out_handle.close()



@app.command()
def summary_sam(
    sam: Path = typer.Option(
        ..., help="SAM file from basecalling and alignment with Dorado"
    ),
    ends: Path = typer.Option(
        ..., help="CSV file with endreasons of each read from `--endreason-fast5` command."
    ),
    output: Path = typer.Option(
        ..., help="Output table of summary values per read in CSV"
    )
):
    """
    Get a table of read identifiers and end reasons from Fast5 (Icarust v0.3.0)
    """

    print("Parsing end-reason file...")
    endreasons = dict()
    # Parse end reasons into dictionary with identifier keys
    with ends.open("r") as end_file:
        for i, line in enumerate(end_file):
            if i > 0:
                content = line.strip().split(",")
                
                read = str(content[0])
                reason = int(content[3])

                endreasons[read] = reason


    print("Summarizing alignment file...")
    out_handle = output.open("w")
    out_handle.write("id,ref,mapq,bp\n")

    summary = dict()
    with sam.open("r") as sam_file:
        for line in sam_file:
            if line.startswith("@"):
                continue

            content = line.strip().split("\t")
            
            read  = str(content[0])
            ref   = str(content[2])
            mapq  = int(content[4])
            bases = len(content[9])

            out_handle.write(f"{read},{ref},{mapq},{bases}\n")

            if ref not in summary.keys():
                summary[ref] = MappingSummary(read_lengths=[])
            else:

                summary[ref].reads += 1
                summary[ref].read_lengths.append(bases)

                if endreasons[read] == 4:
                    summary[ref].unblocked += 1
    
    read_length_density_all(summary, f"read_lengths_density_all.png", min_length=50, max_length=4000, colors=LAPUTA_MEDIUM)
    read_length_histogram_all(summary, f"read_lengths_histogram_all.png", min_length=50, max_length=4000, colors=LAPUTA_MEDIUM)
    read_length_histogram_distinct(summary, f"read_lengths_histogram_distinct.png", min_length=50, max_lengths=[1000, 4000], colors=LAPUTA_MEDIUM)

    for ref, data in summary.items():
        data.mean_length = statistics.mean(data.read_lengths)
        data.median_length = statistics.median(data.read_lengths)
        data.bases = sum(summary[ref].read_lengths)
        data.n50_length = compute_n50(data.read_lengths)
        data.unblocked_percent = (data.unblocked/data.reads)*100
        data.read_lengths = []

    print("Completed statistics summary for alignments")

    pprint(summary)

    out_handle.close()


app()


