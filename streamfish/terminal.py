import typer
from typing import List
from pathlib import Path
from .evaluation import plot_relative_signal, plot_signal_distribution

app = typer.Typer(add_completion=False)


@app.command()
def test():
    """
    Test command-line interface
    """
    typer.echo("ok")

@app.command()
def plot_simulation(
    summaries: List[Path] = typer.Argument(
        ..., help="Directory containing exeriment community meta-data linked simulation signal read summaries (Slow5-like)"
    ), 
    variant: List[str] = typer.Option(
        None, help="Config name glob patterns, use multiple to show as color variants in panels"
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
        "Cumulative signal for members of simulated community", help="Plot output title"
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
    distribution: bool = typer.Option(
        False, help="Plot the distribution plots only"
    ),
):
    """
    Plot community meta-data linked simulation runs and experiment evaluations
    """

    outdir.mkdir(exist_ok=True)

    tags = []
    for t in sim_tags.split(","):
        if t.strip():
            tags.append(t.strip())

    if not distribution:
        plot_relative_signal(
            summaries=summaries, 
            variants=variant,
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

    plot_signal_distribution(
        summaries=summaries, 
        variants=variant,
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


    