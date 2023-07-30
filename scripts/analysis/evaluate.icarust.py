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

from pafpy import PafFile, PafRecord
from dataclasses import dataclass
from typing import List, Dict, Tuple
from pprint import pprint
from collections import Counter

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

@dataclass
class ReferenceData:

    reference: str
    start: int
    end: int
    name: str

    read_lengths_pass: List[int] 
    read_lengths_unblocked: List[int]
    
    reads_pass_records: List[PafRecord] = None
    
    mapping_qualities_pass: List[int] = None
    mapping_qualities_unblocked: List[int] = None

    reads_pass: int = 0
    bases_pass: int = 0

    reads_unblocked: int = 0
    bases_unblocked: int = 0

# Summary for each reference sequence 
# used in alignment
@dataclass
class ReferenceSummary:

    reference: str = ""
    target: str = ""
    start: int = 0
    end: int = 0

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


    def from_mapping_summary(self, data: ReferenceData, control: bool = False):

        self.reference = f"{data.reference}"
        self.target = data.name
        self.start = data.start 
        self.end = data.end

        self.control = control

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

        self.mean_mapq_pass = statistics.mean(data.mapping_qualities_pass) if data.mapping_qualities_pass and len(data.mapping_qualities_pass) > 1 else 0 
        self.mean_length_pass = statistics.mean(data.read_lengths_pass) if data.read_lengths_pass and len(data.read_lengths_pass) > 1 else 0
        self.median_length_pass = int(statistics.median(data.read_lengths_pass)) if data.read_lengths_pass and len(data.read_lengths_pass) > 1 else 0
        self.n50_length_pass = compute_n50(data.read_lengths_pass) if data.read_lengths_pass and len(data.read_lengths_pass) > 1 else 0

        self.mean_mapq_unblocked = statistics.mean(data.mapping_qualities_unblocked) if data.mapping_qualities_unblocked and len(data.mapping_qualities_unblocked) > 1 else 0
        self.mean_length_unblocked = statistics.mean(data.read_lengths_unblocked)  if data.read_lengths_unblocked and len(data.read_lengths_unblocked) > 1 else 0
        self.median_length_unblocked = int(statistics.median(data.read_lengths_unblocked)) if data.read_lengths_unblocked and len(data.read_lengths_unblocked) > 1 else 0
        self.n50_length_unblocked = compute_n50(data.read_lengths_unblocked) if data.read_lengths_unblocked and  len(data.read_lengths_unblocked) > 1 else 0

        return self
    
    def from_mapping_summaries(self, summaries: List[ReferenceData]):

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

            
            if data.read_lengths_unblocked:
                all_lengths_unblocked += data.read_lengths_unblocked

            if data.read_lengths_pass:
                all_lengths_pass += data.read_lengths_pass

            if data.mapping_qualities_unblocked:
                all_mapq_unblocked += data.mapping_qualities_unblocked

            if data.mapping_qualities_pass:
                all_mapq_pass += data.mapping_qualities_pass

        try:
            self.unblocked_percent = (self.reads_unblocked/(self.reads_pass+self.reads_unblocked))*100
        except ZeroDivisionError:
            self.unblocked_percent = 0
            
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

def read_length_density_all(data: Dict[str, ReferenceData], output_file: str, min_length: int =50, max_length: int =4000, colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.6)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, ax = plt.subplots()
    
    for i, (ref, summary) in enumerate(data.items()): 
        lengths = [d for d in summary.read_lengths_unblocked if d >= min_length and d < max_length] + [d for d in summary.read_lengths_pass if d >= min_length and d < max_length]
        sns.kdeplot(lengths, ax=ax, label=ref, color=colors[i+1], fill=True, alpha=0.8)

    # Set the plot labels and title
    ax.set_xlabel('bp')
    ax.set_ylabel('Density')
    ax.set_title('Read length', fontdict={'weight': 'bold'})
    ax.legend()

    sns.despine()
    ax.grid(False)
    
    plt.xlim(0, max_length)
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()


def read_length_histogram_all(data: Dict[str, ReferenceData], output_file: str, min_length: int =50, max_length: int =4000, colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.6)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, ax = plt.subplots()
    
    length_data = {'read_length': [], 'target': []}
    for i, (ref, summary) in enumerate(data.items()): 
        lengths = [d for d in summary.read_lengths_unblocked if d >= min_length and d < max_length] + [d for d in summary.read_lengths_pass if d >= min_length and d < max_length]
        length_data['read_length'] += lengths
        length_data['target'] += [ref for _ in lengths]

    df = pandas.DataFrame.from_dict(length_data)

    sns.histplot(data=df, x="read_length", hue="target", kde=False, ax=ax, bins='auto', fill=True, alpha=0.8)

    # Set the plot labels and title
    ax.set_xlabel('bp')
    ax.set_ylabel('Reads')
    ax.set_title('Read length', fontdict={'weight': 'bold'})

    sns.despine()
    ax.grid(False)
    
    plt.xlim(0, max_length)    
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()


def read_length_histogram_distinct(data: Dict[str, ReferenceData], output_file: str, min_length: int =50, max_lengths: List[int] = [4000], colors: List[str] = LAPUTA_MEDIUM):

    sns.set_style('whitegrid')
    sns.set(style='ticks', font_scale=0.4)

    # Remove unmapped
    data.pop("*", None)

    # Create a figure and axes
    fig, axes = plt.subplots(nrows=1, ncols=len(data))
    
    for i, (ref, summary) in enumerate(data.items()): 
        if len(data) > 1:
            ax = axes[i]
        else:
            ax = axes

        try:
            max_length = max_lengths[i]
        except IndexError:
            max_length = max_lengths[0]

        lengths = [d for d in summary.read_lengths_unblocked if d >= min_length and d < max_length] + [d for d in summary.read_lengths_pass if d >= min_length and d < max_length]
        sns.histplot(lengths, kde=False, ax=ax, color=colors[i+1], bins='auto', fill=True, alpha=1.)

        # Set the plot labels and title
        ax.set_xlabel('bp')
        ax.set_ylabel('Reads')
        ax.set_title(ref, fontdict={'weight': 'bold'})

        sns.despine()
        ax.grid(False)

        ax.set_xlim(0, max_length)    
    
    plt.savefig(output_file, dpi=300, bbox_inches='tight', transparent=False)
    plt.close()



#####################
# SUMMARY FUNCTIONS #
#####################

def get_endreasons(file: Path) -> Dict[str, int]:

    print(f"Parsing end-reasons: {file}")
    endreasons = dict()
    # Parse end reasons into dictionary with identifier keys
    with file.open("r") as end_file:
        for i, line in enumerate(end_file):
            if i > 0:
                content = line.strip().split(",")
                
                read = str(content[0])
                reason = int(content[3])

                endreasons[read] = reason
    return endreasons


def create_reference_summary_dataframe(
    active_summary: Dict[str, ReferenceData], 
    control_summary: Dict[str, ReferenceData] = None, 
    output: Path = None
) -> Tuple[pandas.DataFrame, pandas.DataFrame]:

    # Create the reference summaries for each experiment arm summary

    summary_list: List[ReferenceData] = [data for _, data in active_summary.items()]
    summaries: List[ReferenceSummary] = [ReferenceSummary().from_mapping_summary(data, control=False) for _, data in active_summary.items()]
    
    if control_summary:
        summaries += [ReferenceSummary().from_mapping_summary(data, control=True) for _, data in control_summary.items()]
        summary_list += [data for _, data in control_summary.items()]

    combined = ReferenceSummary(reference="total").from_mapping_summaries(summaries=summary_list)

    df = pandas.DataFrame([o.__dict__ for o in summaries])
    df = df.sort_values(by="reference")
    df_combined = pandas.DataFrame([combined.__dict__])
    
    if output:
        df.to_csv(output, index=False, sep=",", header=True)

    return df, df_combined

def get_region_data_paf(ends: Path, alignment: Path, targets: Path) -> Dict[str, ReferenceData]:

    # Get endreasons and target regions
    endreasons = get_endreasons(file=ends)
    target_regions = pandas.read_csv(targets, sep="\t", header=None, names=["ref", "start", "end", "name"])

    # Setup target region data summaries
    target_region_data: Dict[str, ReferenceData] = {
        f"{row['ref']}::{row['start']}::{row['end']}::{row['name']}": ReferenceData(
            reference=row["ref"], start=row["start"], end=row["end"], name=row["name"], 
            read_lengths_pass=[], read_lengths_unblocked=[], reads_pass_records=[]
        ) for _, row in target_regions.iterrows()
    }

    # Add off_target data summary for each reference
    for _, row in target_regions.iterrows():
        outside = f"{row['ref']}::0::0::off_target"
        if outside not in target_region_data.keys():
            target_region_data[outside] = ReferenceData(
                reference=row['ref'], start=0, end=0, name="off_target", 
                read_lengths_pass=[], read_lengths_unblocked=[], reads_pass_records=[]
            ) 
    
    # Add other off targets that are not specified in the target file
    # but are aligned against - this only considers any off targets
    # that have alignments and may not represent the full simulated
    # input (if no alignments present) - check if it matters [TODO]
    
    add_off_targets_from_alignments(alignment=alignment, target_region_data=target_region_data)

    print(target_region_data)
        
    # In the unblock decision any alignment (primary or secondary)
    # is considered to match the reference or target region. To be 
    # consistent, we add read data to unblocked statistics if any of the 
    # alignments (primary/secondary) for this read matches within a region

    # We therefore have to iterate over the PAF output twice - the first time
    # to identify reads with secondary alignments, and the second time to 
    # extract the correct data for those reads with secondary alignments -
    # if they are prmary only we just go ahead with the standard conditions
    
    reads_with_secondary_alignments = get_reads_with_secondary_alignments(alignment=alignment)

    read_records_secondary = dict()
    with PafFile(alignment) as paf:
        for record in paf:
            if record.qname in reads_with_secondary_alignments:
                # If this read has secondary alignments, do not process it,
                # we evaluate it seperately after doing the primary alignments
                if record.qname not in read_records_secondary:
                    read_records_secondary[record.qname] = [record]
                else:
                    read_records_secondary[record.qname].append(record)

                continue 
            
            evaluate_target_regions_for_reads_with_primary_alignment_only(record=record, target_region_data=target_region_data, endreasons=endreasons)
    
    for _, records in read_records_secondary.items():
        evaluate_target_regions_for_reads_with_secondary_alignments(records=records, target_region_data=target_region_data, endreasons=endreasons)

    print_target_summary(target_region_data=target_region_data)

    return target_region_data

def add_off_targets_from_alignments(alignment: Path, target_region_data: Dict[str, ReferenceData]):

    refs_unique = []
    with PafFile(alignment) as paf:
        for record in paf:
            if record.tname not in refs_unique:
                refs_unique.append(record.tname)

    refs_included = [d.reference for d in target_region_data.values()]
    for ref in refs_unique:
        if ref not in refs_included:
            target_region_data[f"{ref}::0::0::off_target"] = ReferenceData(
                reference=ref, start=0, end=0, name="off_target", read_lengths_pass=[], read_lengths_unblocked=[], reads_pass_records=[]
            ) 


def calculate_average_coverage(paf_records: List[PafRecord], start: int, end: int):
    coverage = {}
    for record in paf_records:
        ref_name = record.tname
        start_pos = record.tstart
        end_pos = record.tend
        for pos in range(max(start, start_pos), min(end, end_pos) + 1):
            coverage[(ref_name, pos)] = coverage.get((ref_name, pos), 0) + 1
    return sum(coverage.values()) / len(coverage)

def get_reads_with_secondary_alignments(alignment: Path) -> List[str]:

    read_align_counter = Counter()
    with PafFile(alignment) as paf:
        for record in paf:
            read_align_counter.update([record.qname])
    
    return [read_id for read_id, count in read_align_counter.items() if count > 1]


def print_target_summary(target_region_data: Dict[str, ReferenceData]):

    # Print on-target / off_target summary of read alignments
    for ref, region_data in target_region_data.items():
        print(region_data.reference, region_data.start, region_data.end, region_data.name, region_data.reads_pass, region_data.reads_unblocked)
        for record in region_data.reads_pass_records:
            if "off_target" not in ref:
                print(f"On-target alignment (pass): {record.tname} {record.tstart} {record.tend} {record.mapq}")
            else:
                # print(f"off_target alignment (pass): {record.tname} {record.tstart} {record.tend} {record.mapq}")
                pass

def evaluate_target_regions_for_reads_with_secondary_alignments(records: List[PafRecord], target_region_data: Dict[str, ReferenceData], endreasons: Dict[str, int]):

    # Only adds a data to the statistics after evaluating primary and secondary alignments for this read

    if not records:
        raise ValueError("Paf record input must have at least one record")

    mapped = False
    for record in records:
        for _, region_data in target_region_data.items():
            if region_data.name == "off_target":
                continue
            # Same condition as in mapping configuration of Streamfish
            if record.tname == region_data.reference and (
                (region_data.start == 0 and region_data.end == 0) or
                ((record.tstart >= region_data.start and record.tstart <= region_data.end) or (record.tend >= region_data.start and record.tend <= region_data.end) or (record.tstart <= region_data.start and record.tend >= region_data.end))
            ):
                mapped = True
                break

        if mapped:
            # If a target alignment is found add statistics for this read
            if endreasons[record.qname] == 4:
                region_data.reads_unblocked += 1
                region_data.bases_unblocked += record.qlen
                region_data.read_lengths_unblocked.append(record.qlen)
            else:
                region_data.reads_pass += 1
                region_data.bases_pass += record.qlen
                region_data.read_lengths_pass.append(record.qlen)
                # Records of passing on-target reads 
                region_data.reads_pass_records.append(record)

            # Stop evaluating alignment records as we found one
            break
    
    # Last record data is used but does not matter since all records have been
    # grouped by read identifier - this uses the primary (first) alignment to 
    # assign to the off_target reference, query name and length (sequence length)
    # are the same for all alignment records
    if not mapped:
        region_data = target_region_data[f"{records[0].tname}::0::0::off_target"]
        if endreasons[records[0].qname] == 4:
            region_data.reads_unblocked += 1
            region_data.bases_unblocked += records[0].qlen
            region_data.read_lengths_unblocked.append(records[0].qlen)
        else:
            region_data.reads_pass += 1
            region_data.bases_pass += records[0].qlen
            region_data.read_lengths_pass.append(records[0].qlen)


def evaluate_target_regions_for_reads_with_primary_alignment_only(record: PafRecord, target_region_data: Dict[str, ReferenceData], endreasons: Dict[str, int]):
    
    mapped = False
    for _, region_data in target_region_data.items():
        if region_data.name == "off_target":
            continue

        # Same condition as in mapping configuration of Streamfish - one case is specified in file where start=0 and end=0 => without region specifications e.g. in broad targeted experiment
        if record.tname == region_data.reference and (
            (region_data.start == 0 and region_data.end == 0) or
            ((record.tstart >= region_data.start and record.tstart <= region_data.end) or (record.tend >= region_data.start and record.tend <= region_data.end) or (record.tstart <= region_data.start and record.tend >= region_data.end))
        ):
            if endreasons[record.qname] == 4:
                region_data.reads_unblocked += 1
                region_data.bases_unblocked += record.qlen
                region_data.read_lengths_unblocked.append(record.qlen)
            else:
                region_data.reads_pass += 1
                region_data.bases_pass += record.qlen
                region_data.read_lengths_pass.append(record.qlen)
                # Records of passing on-target reads 
                region_data.reads_pass_records.append(record)

            # We break here as a read should only map into one region -
            # in the block unblock decision as we any region mapping and
            # make a decision - done here this way so we don't overcount
            mapped = True
            break
    
    # If it falls outside any target region:
    if not mapped:
        region_data = target_region_data[f"{record.tname}::0::0::off_target"]
        if endreasons[record.qname] == 4:
            region_data.reads_unblocked += 1
            region_data.bases_unblocked += record.qlen
            region_data.read_lengths_unblocked.append(record.qlen)
        else:
            region_data.reads_pass += 1
            region_data.bases_pass += record.qlen
            region_data.read_lengths_pass.append(record.qlen)

def plot_target_coverage_panel(target_regions: pandas.DataFrame):

    n_regions = len(target_regions)

    # Create a figure and axes
    fig, axes = plt.subplots(nrows=n_regions//3, ncols=3)

    for (i, row) in target_regions.iterrows():
        pass


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

    # TODO: multi-thread this - too slow on large datasets

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
    active_ends: Path = typer.Option(
        ..., help="CSV file with endreasons from `--endreason-fast5`"
    ),
    active_paf:  Path = typer.Option(
        ..., help="PAF file from alignment with minimap2"
    ),
    target_regions: Path = typer.Option(
        ..., help="Target regions file for the experiment"
    ),
    outdir_plots: Path = typer.Option(
        ..., help="Output directory for figures"
    ),
    control_paf: Path = typer.Option(
        None, help="CONTROL SAM file from basecalling and alignment with Dorado"
    ),
    control_ends: Path = typer.Option(
        None, help="CONTROL CSV file with endreasonsfrom `--endreason-fast5`"
    ),
):
    """
    Get a table of read identifiers and end reasons from Fast5 (Icarust v0.3.0)
    """
       
    active_summary = get_region_data_paf(ends=active_ends, alignment=active_paf, targets=target_regions)

    control_summary = None
    if control_paf and control_ends:
        control_summary =  get_region_data_paf(ends=control_ends, alignment=control_paf, targets=target_regions)
    
    df, df_combined = create_reference_summary_dataframe(active_summary=active_summary, control_summary=control_summary, output=summary_table)

    with pandas.option_context(
        'display.max_rows', None,
        'display.max_columns', None,
        'display.precision', 2
    ):
        print(df)
        # print(df_combined)
    
    
    read_length_density_all(active_summary, outdir_plots / f"read_lengths_density_all.png", min_length=50, max_length=40000, colors=LAPUTA_MEDIUM)
    read_length_histogram_all(active_summary, outdir_plots / f"read_lengths_histogram_all.png", min_length=50, max_length=40000, colors=LAPUTA_MEDIUM)
    read_length_histogram_distinct(active_summary, outdir_plots / f"read_lengths_histogram_distinct.png", min_length=50, max_lengths=[1000, 40000], colors=LAPUTA_MEDIUM)



app()


