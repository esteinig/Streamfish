"""
Important to run the --device cuda:all:100% option on the server!
"""

import sys
import typer
import time
import logging
import threading

import numpy as np
from pyguppy_client_lib.pyclient import PyGuppyClient

guppy_logger = logging.getLogger("OntCaller")
app = typer.Typer(add_completion=False)


@app.command()
def guppy_client(
        address: str = typer.Option(
            ..., help="IPC or TCP address for Dorado or Guppy server"
        ),
        config: str = typer.Option(
            "dna_r9.4.1_450bps_fast", help="Configuration name to load"
        ),
        throttle: float = typer.Option(
            0.01, help="Client throttle in seconds"
        ),
        threads: int = typer.Option(
            4, help="Client threads"  # important, test this
        ),
        max_reads_queued: int = typer.Option(
            2048, help="Maximum number of reads in queue"
        ),
):
    """
    Parse signal data from input stream, submit to Guppy server and emit results to standard output
    """

    client = PyGuppyClient(
        address=address,
        config=config,
        throttle=throttle,
        retries=5,
        num_client_threads=threads
    )

    client.set_params({
        "priority": PyGuppyClient.high_priority,
        "max_reads_queued": max_reads_queued
    })

    client.connect()

    def read_guppy_inputs(caller: PyGuppyClient):

        while True:
            read_count = 0
            for line in sys.stdin:
                line = line.rstrip('\n').split()

                if not line:
                    continue
                
                input_data = dict(
                    read_tag=read_count,
                    read_id=line[0],
                    raw_data=np.array(line[7:], dtype=np.dtype('i2')),
                    daq_offset=float(line[4]),
                    daq_scaling=float(line[5]) / float(line[3])
                )   

                _ = caller.pass_read(input_data)

                read_count += 1

    input_thread = threading.Thread(target=read_guppy_inputs, args=(client,))
    input_thread.daemon = True 
    input_thread.start()

    while True:
        reads = client.get_completed_reads()

        # Check if necessary - seems good 
        if not reads and throttle > 0:
            time.sleep(client.throttle)
            continue 

        for read in reads:
            for r in read:
                sys.stdout.write(f"{r['metadata']['read_id']}\n{r['datasets']['sequence']}\n+\n-\n") # fake fastq no qual for mapping 
                sys.stdout.flush()

app()