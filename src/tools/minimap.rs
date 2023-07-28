//! Mapping tool implementing `minimap2-rs` for evaluation pipeline
//! 
//! This is the multi-threaded version from @jguhlin's amazing `minimap2-rs` 
//! here: https://github.com/jguhlin/minimap2-rs/tree/main/fakeminimap2 
//! implemented as a struct with mapping functions



use std::mem;
use minimap2::*;
use log::LevelFilter::*;
use std::sync::{Arc, Mutex};

use crossbeam::queue::ArrayQueue;
use fern::colors::{Color, ColoredLevelConfig};
use needletail::parse_fastx_file;
use needletail::{FastxReader, Sequence};
use std::time::{Duration, SystemTime};

enum WorkQueue<T> {
    Work(T),
    Result(T),
}

struct Minimapper {
    pub index: PathBuf,
    pub aligner: Aligner,
}
impl Minimapper {
    pub fn new(index: PathBuf) -> Self {
        // Create aligner, can customize options on initiation

        let aligner = Aligner::builder()
            .map_ont()
            .with_cigar()
            .with_index("hg38_chr_M.mmi", None)
            .expect("Unable to build index");

    }
    pub fn map(fastx: PathBuf) {

        let mut reader: Box<dyn FastxReader> = parse_fastx_file(fastx).unwrap();

        let work_queue = Arc::new(ArrayQueue::<WorkQueue<String>>::new(20000));
        let results_queue = Arc::new(ArrayQueue::<WorkQueue<Vec<Mapping>>>::new(20000));

        // Routine 1: push sequences from file onto sequence data queue accessible from threads

        let sequences = Arc::new(Mutex::new(vec![]));
        let mut x = sequences.lock().unwrap();
        while let Some(record) = reader.next() {
            let record = record.unwrap();
            x.push(format!(
                ">{}\n{}",
                String::from_utf8(record.id().to_vec()).unwrap(),
                String::from_utf8(record.sequence().to_vec()).unwrap()
            ));
        }
        mem::drop(x);

        // Routine 2: spawn alignment threads that pull from the work queue and push into 

        for _ in 0..8 {
            let work_queue = Arc::clone(&work_queue);
            let results_queue = Arc::clone(&results_queue);
            debug!("Get clones");
            let aligner = aligner.clone();
            debug!("Cloned aligner");
            std::thread::spawn(move || loop {
                let backoff = crossbeam::utils::Backoff::new();
                let work = work_queue.pop();
                match work {
                    Some(WorkQueue::Work(sequence)) => {
                        let alignment = aligner
                            .map(sequence.as_bytes(), true, false, None, None)
                            .expect("Unable to align");
                        match results_queue.push(WorkQueue::Result(alignment)) {
                            Ok(()) => {}
                            Err(_) => {
                                backoff.snooze();
                            }
                        }
                    }
                    Some(_) => {}
                    None => {
                        backoff.snooze();
                    }
                }
            });
        }
        let sequences_borrow = Arc::clone(&sequences);


    }

}

/// Transform a nucleic acid sequence into its "normalized" form.
///
/// The normalized form is:
///  - only AGCTN and possibly - (for gaps)
///  - strip out any whitespace or line endings
///  - lowercase versions of these are uppercased
///  - U is converted to T (make everything a DNA sequence)
///  - some other punctuation is converted to gaps
///  - IUPAC bases may be converted to N's depending on the parameter passed in
///  - everything else is considered a N
pub fn normalize(seq: &[u8]) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::with_capacity(seq.len());
    let mut changed: bool = false;

    for n in seq.iter() {
        let (new_char, char_changed) = match (*n, false) {
            c @ (b'A', _) | c @ (b'C', _) | c @ (b'G', _) | c @ (b'T', _) => (c.0, false),
            (b'a', _) => (b'A', true),
            (b'c', _) => (b'C', true),
            (b'g', _) => (b'G', true),
            // normalize uridine to thymine
            (b't', _) | (b'u', _) | (b'U', _) => (b'T', true),
            // normalize gaps
            (b'.', _) | (b'~', _) => (b'-', true),
            // remove all whitespace and line endings
            (b' ', _) | (b'\t', _) | (b'\r', _) | (b'\n', _) => (b' ', true),
            // everything else is an N
            _ => (b' ', true),
        };
        changed = changed || char_changed;
        if new_char != b' ' {
            buf.push(new_char);
        }
    }
    if changed {
        Some(buf)
    } else {
        None
    }
}

fn main() {

    let aligner = Aligner::builder()
        .map_ont()
        .with_cigar()
        .with_index("hg38_chr_M.mmi", None)
        .expect("Unable to build index");

    let mut reader: Box<dyn FastxReader> = parse_fastx_file("testing_fake_minimap2_chrM.fasta")
        .unwrap_or_else(|_| panic!("Can't find FASTA file at testing_r10_fasta.fasta"));

    let work_queue = Arc::new(ArrayQueue::<WorkQueue<String>>::new(20000));
    let results_queue = Arc::new(ArrayQueue::<WorkQueue<Vec<Mapping>>>::new(20000));
    // TODO: Make threads clap argument

    // spawn 8 threads
    info!("Spawnging threads");
    for _ in 0..8 {
        let work_queue = Arc::clone(&work_queue);
        let results_queue = Arc::clone(&results_queue);
        debug!("Get clones");
        let aligner = aligner.clone();
        debug!("Cloned aligner");
        std::thread::spawn(move || loop {
            let backoff = crossbeam::utils::Backoff::new();
            let work = work_queue.pop();
            match work {
                Some(WorkQueue::Work(sequence)) => {
                    let alignment = aligner
                        .map(sequence.as_bytes(), true, false, None, None)
                        .expect("Unable to align");
                    match results_queue.push(WorkQueue::Result(alignment)) {
                        Ok(()) => {}
                        Err(_) => {
                            backoff.snooze();
                        }
                    }
                }
                Some(_) => {}
                None => {
                    backoff.snooze();
                }
            }
        });
    }
    let sequences_borrow = Arc::clone(&sequences);

    let wq = Arc::clone(&work_queue);
    // create threead and just spam push things onto the to be mapped queue
    std::thread::spawn(move || loop {
        let z = &sequences_borrow.lock().unwrap();
        for seq in z.iter() {
            match wq.push(WorkQueue::Work(seq.clone())) {
                Ok(()) => {}
                Err(_) => {
                    // queue full, sleep and try again
                    std::thread::sleep(Duration::from_millis(1));
                }
            };
        }
    });
    // Loop and pull results down
    let mut num_alignments: usize = 0;
    loop {
        debug!("{}", results_queue.len());
        let result = results_queue.pop();
        match result {
            Some(WorkQueue::Result(_alignment)) => num_alignments += 1,
            Some(_) => {
                error!("Found a random variant in the results queue")
            }
            None => {
                warn!("Popped nothing");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(1));
        info!("Iteration over, total alignments {}", num_alignments);
    }
    // Work thread
}