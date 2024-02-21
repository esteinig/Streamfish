from setuptools import setup, find_packages

setup(
    name="streamfish",
    url="https://github.com/esteinig/streamfish",
    author="Eike J. Steinig",
    author_email="eike.steinig@unimelb.edu.au",
    packages=find_packages(),
    include_package_data=True,
    install_requires=[
        "typer",
        "pyslow5",
        "ridgeplot"
    ],
    entry_points="""
        [console_scripts]
        streamfish-utils=streamfish.terminal:app
    """,
    version="0.1.0",
    license="MIT",
    description="Streamfish utility library for experiment plotting and other shenaningans",
)