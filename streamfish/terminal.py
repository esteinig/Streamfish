import typer

from pathlib import Path
from .evaluation import plot_relative_signal

app = typer.Typer(add_completion=False)


@app.command()
def test():
    """
    Test command-line interface
    """
    typer.echo("ok")

@app.command()
def plot_simulation(
    experiments: Path = typer.Option(
        ..., help="Directory containing exeriment community meta-data linked simulation signal read summaries (Slow5-like)"
    ), 
    controls: Path = typer.Option(
        None, help="Directory containing control community meta-data linked simulation signal read summaries (Slow5-like)"
    ), 
    exp_glob: str = typer.Option(
        "*__experiment__*.tsv", help="Experiment file name glob pattern"
    ), 
    ctrl_glob: str = typer.Option(
        "*__control__*.tsv", help="Experiment file name glob pattern"
    ), 
    outdir: Path = typer.Option(
        Path.cwd(), help="Output directory for plot files"
    ), 
    prefix: str = typer.Option(
        "simulation", help="Plot output file prefix"
    ), 
    interval: int = typer.Option(
        10, help="Interval in seconds for time slices in cumulative signal plots"
    ), 
    title: str = typer.Option(
        "Relative cumlative signal of simulated microbiome", help="Plot output title"
    ), 
    size: str = typer.Option(
        "30,12", help="Plot dimensions as comma-delimited tuple e.g. 18,12"
    ),
    format: str = typer.Option(
        "pdf", help="Plot output file format"
    ),
    host_chr: bool = typer.Option(
        False, help="Summarize reference contigs starting with 'chr' as 'Host' in cumulative signal plots"
    ),
    sim_tags: str = typer.Option(
        "", help="Comma-delimited str of str for each input file to tag members in cumulative signal plots"
    ),
    diff_limit: float = typer.Option(
        600, help="Comma-delimited str of str for each input file to tag members in cumulative signal plots"
    ),

):
    """
    Plot community meta-data linked simulation runs and experiment evaluations
    """

    tags = []
    for t in sim_tags.split(","):
        if t.strip():
            tags.append(t.strip())

    exp_paths = [
        p for p in experiments.glob(exp_glob)
    ]
    
    ctrl_paths = [
        p for p in controls.glob(ctrl_glob)
    ] if controls else [
        p for p in experiments.glob(ctrl_glob)
    ]
    
    plot_relative_signal(
        experiments=exp_paths, 
        controls=ctrl_paths,
        outdir=outdir, 
        prefix=prefix, 
        title=title, 
        plot_size=size, 
        plot_format=format, 
        by_chromosome=not host_chr, 
        interval=interval, 
        sim_tags=tags,
        diff_limit=diff_limit
    )


    