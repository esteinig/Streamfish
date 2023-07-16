from setuptools import setup, find_packages

setup(
    name="deepfish",
    url="https://github.com/esteinig/streamfish",
    author="Eike J. Steinig",
    author_email="eikejoachim.steinig@my.jcu.edu.au",
    packages=find_packages(),
    include_package_data=True,
    install_requires=[
        "typer"
    ],
    entry_points="""
        [console_scripts]
        deepfish=deepfish.terminal:app
    """,
    version="0.1.0",
    license="MIT",
    description="Deepfish is a platform to train and evaluate neural networks for nanopore signal classification",
)