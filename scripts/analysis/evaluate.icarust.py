"""
Extracts the Icarust v0.3.0 (53ca111a3b20563a2c9ca22bba766aef2d95deb5) endreasons from Fast5

0 = Default
1 = Positive Signal
4 = Unblock

pip install typer ont-fast5-api seaborn matplotlib

"""

import typer
import statistics

import pandas
import seaborn as sns
import matplotlib.pyplot as plt

from dataclasses import dataclass
from typing import List, Dict, Tuple
from pprint import pprint

from pathlib import Path
from ont_fast5_api.fast5_interface import get_fast5_file


#####################
#  HELPER STRUCTS  #
#####################

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

# Summary 
@dataclass
class MappingSummary:

    reference: str

    read_lengths_pass: List[int]
    mapping_qualities_pass: List[int]

    read_lengths_unblocked: List[int]
    mapping_qualities_unblocked: List[int]

    reads_pass: int = 0
    bases_pass: int = 0

    reads_unblocked: int = 0
    bases_unblocked: int = 0

# Summary for each reference sequence 
# used in alignment
@dataclass
class ReferenceSummary:
    reference: str = ""
    control: bool = False

    reads_total: int = 0
    bases_total: int = 0

    reads_pass: int = 0
    bases_pass: int = 0
    n50_length_pass: int = 0
    median_length_pass: int = 0
    mean_length_pass: float = 0.
    mean_mapq_pass: float = 0.

    reads_unblocked: int = 0
    bases_unblocked: int = 0
    n50_length_unblocked: int = 0
    median_length_unblocked: int = 0
    mean_length_unblocked: float = 0.
    mean_mapq_unblocked: float = 0.

    unblocked_percent: float = 0.


    def from_mapping_summary(self, data: MappingSummary, control: bool = False):

        self.reference=data.reference
        self.control=control

        self.reads_total = data.reads_pass+data.reads_unblocked
        self.bases_total = data.bases_pass+data.bases_unblocked

        self.reads_pass = data.reads_pass
        self.bases_pass = data.bases_pass
        self.reads_unblocked = data.reads_unblocked
        self.bases_unblocked = data.bases_unblocked

        try:
            self.unblocked_percent = (data.reads_unblocked/(data.reads_pass+data.reads_unblocked))*100
        except ZeroDivisionError:
            self.unblocked_percent = 0

        self.mean_mapq_pass = statistics.mean(data.mapping_qualities_pass) if len(data.mapping_qualities_pass) > 1 else 0
        self.mean_length_pass = statistics.mean(data.read_lengths_pass) if len(data.read_lengths_pass) > 1 else 0
        self.median_length_pass = int(statistics.median(data.read_lengths_pass)) if len(data.read_lengths_pass) > 1 else 0
        self.n50_length_pass = compute_n50(data.read_lengths_pass) if len(data.read_lengths_pass) > 1 else 0

        self.mean_mapq_unblocked = statistics.mean(data.mapping_qualities_unblocked) if len(data.mapping_qualities_unblocked) > 1 else 0
        self.mean_length_unblocked = statistics.mean(data.read_lengths_unblocked)  if len(data.read_lengths_unblocked) > 1 else 0
        self.median_length_unblocked = int(statistics.median(data.read_lengths_unblocked)) if len(data.read_lengths_unblocked) > 1 else 0
        self.n50_length_unblocked = compute_n50(data.read_lengths_unblocked)  if len(data.read_lengths_unblocked) > 1 else 0

        return self
    
    def from_mapping_summaries(self, summaries: List[MappingSummary]):

        self.reference=""
        self.control=False

        all_lengths_unblocked = []
        all_lengths_pass = []
        all_mapq_unblocked = []
        all_mapq_pass = []

        for data in summaries:


            self.reads_total += data.reads_pass+data.reads_unblocked
            self.bases_total += data.bases_pass+data.bases_unblocked

            self.reads_pass += data.reads_pass
            self.bases_pass += data.bases_pass
            self.reads_unblocked += data.reads_unblocked
            self.bases_unblocked += data.bases_unblocked

            
            all_lengths_unblocked += data.read_lengths_unblocked
            all_lengths_pass += data.read_lengths_pass
            all_mapq_unblocked += data.mapping_qualities_unblocked
            all_mapq_pass += data.mapping_qualities_pass

        self.unblocked_percent = (self.reads_unblocked/(self.reads_pass+self.reads_unblocked))*100
        
        self.mean_mapq_pass = statistics.mean(all_mapq_pass) if len(all_mapq_pass) > 1 else 0
        self.mean_length_pass = statistics.mean(all_lengths_pass) if len(all_lengths_pass) > 1 else 0
        self.median_length_pass = int(statistics.median(all_lengths_pass)) if len(all_lengths_pass) > 1 else 0
        self.n50_length_pass = compute_n50(all_lengths_pass) if len(all_lengths_pass) > 1 else 0

        self.mean_mapq_unblocked = statistics.mean(all_mapq_unblocked) if len(all_mapq_unblocked) > 1 else 0
        self.mean_length_unblocked = statistics.mean(all_lengths_unblocked)  if len(all_lengths_unblocked) > 1 else 0
        self.median_length_unblocked = int(statistics.median(all_lengths_unblocked)) if len(all_lengths_unblocked) > 1 else 0
        self.n50_length_unblocked = compute_n50(all_lengths_unblocked)  if len(all_lengths_unblocked) > 1 else 0

        return self

######################
#  HELPER FUNCTIONS  #
######################


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

########################
#  PLOTTING FUNCTIONS  #
########################

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
    ax.set_title('Read lengt', fontdict={'weight': 'bold'})

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



#####################
# SUMMARY FUNCTIONS #
#####################

def get_summary(ends: Path, sam: Path, output: Path = None) -> Dict[str, MappingSummary]:

    print(f"Parsing end-reasons: {ends}")
    endreasons = dict()
    # Parse end reasons into dictionary with identifier keys
    with ends.open("r") as end_file:
        for i, line in enumerate(end_file):
            if i > 0:
                content = line.strip().split(",")
                
                read = str(content[0])
                reason = int(content[3])

                endreasons[read] = reason


    print(f"Summarizing mappings: {sam}")
    print(f"Writing summary data to: {output}")

    if output is not None:
        out_handle = output.open("w")
        out_handle.write("id,ref,mapq,bp\n")

    summary: Dict[str, MappingSummary] = dict()
    with sam.open("r") as sam_file:
        for line in sam_file:
            if line.startswith("@"):
                continue

            content = line.strip().split("\t")
            
            read  = str(content[0])
            ref   = str(content[2])
            mapq  = int(content[4])
            bases = len(content[9])

            if output is not None:
                out_handle.write(f"{read},{ref},{mapq},{bases}\n")

            if ref not in summary.keys():
                summary[ref] = MappingSummary(
                    reference=ref, read_lengths_pass=[], mapping_qualities_pass=[],
                    read_lengths_unblocked=[], mapping_qualities_unblocked=[]
                )
            else:

                if endreasons[read] == 4:
                    summary[ref].reads_unblocked += 1
                    summary[ref].bases_unblocked += bases
                    summary[ref].read_lengths_unblocked.append(bases)
                    summary[ref].mapping_qualities_unblocked.append(mapq)
                else:
                    summary[ref].reads_pass += 1
                    summary[ref].bases_pass += bases
                    summary[ref].read_lengths_pass.append(bases)
                    summary[ref].mapping_qualities_pass.append(mapq)


    if output is not None:
        out_handle.close()
        
    return summary

def create_reference_summary_dataframe(
    active_summary: Dict[str, MappingSummary], 
    control_summary: Dict[str, MappingSummary] = None, 
    output: Path = None
) -> Tuple[pandas.DataFrame, pandas.DataFrame]:

    # Create the reference summaries for each experiment arm summary

    summary_list: List[MappingSummary] = [data for _, data in active_summary.items()]
    summaries: List[ReferenceSummary] = [ReferenceSummary().from_mapping_summary(data, control=False) for _, data in active_summary.items()]
    
    if control_summary:
        summaries += [ReferenceSummary().from_mapping_summary(data, control=True) for _, data in control_summary.items()]
        summary_list += [data for _, data in control_summary.items()]

    combined = ReferenceSummary().from_mapping_summaries(summaries=summary_list)

    df = pandas.DataFrame([o.__dict__ for o in summaries])
    df = df.sort_values(by="reference")

    df_combined = pandas.DataFrame([combined.__dict__])
    
    if output:
        df.to_csv(output, index=False, sep=",", header=True)

    return df, df_combined

########################
# TERMINAL APPLICATION #
########################

app = typer.Typer(add_completion=False)


@app.command()
def endreason(
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
def evaluation(
    summary_table: Path = typer.Option(
        ..., help="Summary metrics table for reference alignments"
    ),
    active_sam: Path = typer.Option(
        ..., help="SAM file from basecalling and alignment with Dorado"
    ),
    active_ends: Path = typer.Option(
        ..., help="CSV file with endreasons from `--endreason-fast5`"
    ),
    active_output: Path = typer.Option(
        None, help="Output table of summary values per read in CSV"
    ),
    control_sam: Path = typer.Option(
        None, help="CONTROL SAM file from basecalling and alignment with Dorado"
    ),
    control_ends: Path = typer.Option(
        None, help="CONTROL CSV file with endreasonsfrom `--endreason-fast5`"
    ),
    control_output: Path = typer.Option(
        None, help="Output table of summary values per read in CSV"
    )
):
    """
    Get a table of read identifiers and end reasons from Fast5 (Icarust v0.3.0)
    """

    active_summary = get_summary(ends=active_ends, sam=active_sam, output=active_output)

    control_summary = None
    if control_sam and control_ends:
        control_summary = get_summary(ends=control_ends, sam=control_sam, output=control_output)
    
    df, df_combined = create_reference_summary_dataframe(active_summary=active_summary, control_summary=control_summary, output=summary_table)

    with pandas.option_context(
        'display.max_rows', None,
        'display.max_columns', None,
        'display.precision', 2
    ):
        print(df)
        print(df_combined)
    
    
    # read_length_density_all(summary, f"read_lengths_density_all.png", min_length=50, max_length=4000, colors=LAPUTA_MEDIUM)
    # read_length_histogram_all(summary, f"read_lengths_histogram_all.png", min_length=50, max_length=4000, colors=LAPUTA_MEDIUM)
    # read_length_histogram_distinct(summary, f"read_lengths_histogram_distinct.png", min_length=50, max_lengths=[1000, 4000], colors=LAPUTA_MEDIUM)



app()


