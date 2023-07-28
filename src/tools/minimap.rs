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

pub struct Minimapper {
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

        let input_work_queue = Arc::clone(&work_queue);
        let align_work_queue = Arc::clone(&input_work_queue);  // linked to input work queue because we need access in thread loops

        let mut reader: Box<dyn FastxReader> = parse_fastx_file(fastx).unwrap();
              
        // Thread to spam push sequences onto the work queue

        std::thread::spawn(move || {

            while let Some(record) = reader.next() {
                let record = record.unwrap();
                log::debug!("Sending read to work queue");
                let seq = format!(
                    ">{}\n{}",
                    String::from_utf8(record.id().to_vec()).unwrap(),
                    String::from_utf8(record.sequence().to_vec()).unwrap()
                );
                match input_work_queue.push(WorkQueue::Work(seq)) {
                    Err(_) => {
                        log::debug!("Work queue is full, sleep and try again...");
                        std::thread::sleep(Duration::from_millis(1));
                    },
                    _ => log::debug!("Sent read to work queue")
                }
            }

            log::info!("Submitted all input reads to alignment threads");
            input_sender.send(true).expect("Failed to send input completion message to main loop");

        });

        // A bunch of alignment threads reading from the work queue
        let mut alignment_thread_senders: Vec<crossbeam::channel::Sender<bool>> = Vec::new();
        for thread_num in 0..threads {
            
            log::info!("Spawning alignment thread #{}", thread_num+1);

            let (thread_sender, thread_receiver) = bounded(1);
            alignment_thread_senders.push(thread_sender);

            let aligner = self.aligner.clone();
            let align_results_queue = Arc::clone(&results_queue);
            let _align_work_queue = Arc::clone(&align_work_queue);
            
            
            std::thread::spawn(move || {
                let backoff = crossbeam::utils::Backoff::new();
                while let Some(work) = _align_work_queue.pop() {

                    if let Ok(_) = thread_receiver.recv() {
                        log::warn!("Thread received termination signal, closing thread #{thread_num}");
                        break;
                    }

                    match work {
                        WorkQueue::Work(sequence) => {
                            log::info!("Received read from work queue, aligning...");
                            let alignment = aligner
                                .map(sequence.as_bytes(), true, false, None, None)
                                .expect("Unable to align");
                            align_results_queue.push(WorkQueue::Result(alignment));
                        }
                        _ => backoff.snooze()
                    };

                    
                    
                }
                        // match work {
                        //     Some(WorkQueue::Work(sequence)) => {
                        //         log::debug!("Received read from work queue, aligning...");
                        //         let alignment = aligner
                        //             .map(sequence.as_bytes(), true, false, None, None)
                        //             .expect("Unable to align");
    
                        //         match align_results_queue.push(WorkQueue::Result(alignment)) {
                        //             Err(_) => {
                        //                 log::info!("Error reading read from work queue, backing off...");
                        //                 backoff.snooze();
                        //             },
                        //             _ => log::info!("Pushed alignment into results queue")
                        //         }
                        //     }
                        //     None => {
                        //         log::info!("No read received from work queue, backing off...");
                        //         backoff.snooze();
                        //     },
                        //     _ => {}
                        // }
                        // if let Ok(_) = thread_receiver.recv() {
                        //     log::warn!("Thread received termination signal, closing thread #{thread_num}");
                        //     break;
                        // }
                        
                    log::warn!("Alignment loop ended, closing thread #{}", thread_num);
                }
            );
        }

        let mut num_alignments: usize = 0;
            
        loop {
            let result = results_queue.pop();

            match result {
                Some(WorkQueue::Result(_alignment)) => {
                    num_alignments += 1;
                    log::warn!("Looping back");
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

            // if let Ok(_) = input_receiver.recv() {
            //     log::warn!("Completed read input into alignment queues");
            //     log::info!("Waiting to drain work queue before sending termination signals");
            //     loop {
            //         let jobs = work_queue.len();
            //         if jobs > 0 {
            //             log::warn!("Jobs in work queue: {jobs}");
            //             std::thread::sleep(std::time::Duration::from_secs(1));
            //         } else {
            //             break;
            //         }
            //     }
            //     log::warn!("Jobs completed, sending termination signals to alignment threads...");
            //     for (i, sender) in alignment_thread_senders.iter().enumerate() {
            //         sender.send(true).expect(&format!("Failed to send termination signal into alignment thread #{i}"));
            //     }
            //     break;
            // }

        }
            

    }

}
