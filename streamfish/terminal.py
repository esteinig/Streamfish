import typer

app = typer.Typer(add_completion=False)


@app.command()
def test():
    """
    Test some functions
    """

    print("Testing some functions")


