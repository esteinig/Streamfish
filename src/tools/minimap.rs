//! Mapping tool implementing `minimap2-rs` for evaluation pipeline
//! 
//! This is the multi-threaded version from @jguhlin's amazing `minimap2-rs` 
//! here: https://github.com/jguhlin/minimap2-rs/tree/main/fakeminimap2 
//! implemented as a struct with mapping functions


use minimap2::*;
use std::sync::Arc;

use std::path::PathBuf;
use std::time::Duration;
use crossbeam::queue::ArrayQueue;
use crossbeam::channel::bounded;

use needletail::parse_fastx_file;
use needletail::{FastxReader, Sequence};

enum WorkQueue<T> {
    Work(T),
    Result(T),
}

struct Minimapper {
    pub aligner: Aligner,
}
impl Minimapper {
    pub fn new(index: PathBuf) -> Self {

        // Create aligner, can customize options on initiation
        let aligner = Aligner::builder()
            .map_ont()
            .with_cigar()
            .with_index(index, None)
            .expect("Unable to build index");

        Self { aligner }
    }
    pub fn map(&self, fastx: PathBuf, threads: u32) {

        let (input_sender, input_receiver) = bounded(1);

        let work_queue = Arc::new(ArrayQueue::<WorkQueue<String>>::new(20000));
        let results_queue = Arc::new(ArrayQueue::<WorkQueue<Vec<Mapping>>>::new(20000));

        let mut reader: Box<dyn FastxReader> = parse_fastx_file(fastx).unwrap();
              
        // Thread to spam push sequences onto the work queue
        let input_work_queue = Arc::clone(&work_queue);
        std::thread::spawn(move || {

            while let Some(record) = reader.next() {
                let record = record.unwrap();
                let seq = format!(
                    ">{}\n{}",
                    String::from_utf8(record.id().to_vec()).unwrap(),
                    String::from_utf8(record.sequence().to_vec()).unwrap()
                );
                if let Err(_) = input_work_queue.push(WorkQueue::Work(seq)) {
                    // Queue full, sleep and try again
                    std::thread::sleep(Duration::from_millis(1));
                }
            }

            log::info!("Submitted all input reads to alignment threads");
            input_sender.send(true).expect("Failed to send input completion message to main loop");

        });

        // A bunch of alignment threads reading from the work queue
        let mut alignment_thread_senders = Vec::new();
        for thread_num in 0..threads {
            
            log::info!("Spawning alignment thread #{thread_num}");

            let (thread_sender, thread_receiver) = bounded(1);
            alignment_thread_senders.push(thread_sender);

            let aligner = self.aligner.clone();
            let align_work_queue = Arc::clone(&work_queue);
            let align_results_queue = Arc::clone(&results_queue);
            
            
            std::thread::spawn(move || loop {
                let backoff = crossbeam::utils::Backoff::new();
                let work = align_work_queue.pop();
                match work {
                    Some(WorkQueue::Work(sequence)) => {
                        // Align sequence from work queue
                        let alignment = aligner
                            .map(sequence.as_bytes(), true, false, None, None)
                            .expect("Unable to align");

                        // Failure to push alignment result into results queue
                        if let Err(_) = align_results_queue.push(WorkQueue::Result(alignment)) {
                            backoff.snooze();
                        }
                    }
                    None => {
                        backoff.snooze();
                    },
                    _ => {}
                }
                if let Ok(_) = thread_receiver.recv() {
                    log::warn!("Thread received termination signal, closing thread #{thread_num}");
                    break;
                }
            });
        }

        let mut num_alignments: usize = 0;
        loop {
            let result = results_queue.pop();
            match result {
                Some(WorkQueue::Result(_alignment)) => {

                    num_alignments += 1
                },
                None => {
                    log::warn!("Received no alignments from the results queue");
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Some(_) => {
                    log::error!("Found a random variant in the results queue - this should not happen")
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
            log::info!("Iteration over, total alignments {}", num_alignments);

            if let Ok(_) = input_receiver.recv() {
                log::warn!("Completed read input into alignment queues");
                log::info!("Waiting for work queue before sending termination signals");
                loop {
                    let jobs = work_queue.len();
                    if jobs > 0 {
                        log::warn!("Jobs in work queue: {jobs}");
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    } else {
                        break;
                    }
                }
                log::warn!("Jobs completed, sending termination signals to alignment threads...");
                for (i, sender) in alignment_thread_senders.iter().enumerate() {
                    sender.send(true).expect(&format!("Failed to send termination signal into alignment thread #{i}"));
                }
                break;
            }
        }
        log::info!("Alignment has been completed")


    }

}
