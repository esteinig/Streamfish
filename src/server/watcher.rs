use notify::EventKind;
use notify::event::CreateKind;
use crate::server::launcher::WorkflowLauncher;
use notify::{Config, PollWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::thread;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub channel: String,
    pub token: String
}

pub fn watch_production<P: AsRef<Path>>(watch_path: P, interval: Duration, timeout: Duration, timeout_interval: Duration) -> notify::Result<()> {
    
    let path_name = "main";

    let (tx, rx) = std::sync::mpsc::channel();
    let tx_c = tx.clone();

    let mut watcher = PollWatcher::new(
        move |watch_event| {
            tx_c.send(watch_event).unwrap();
        },
        Config::default().with_poll_interval(interval)
    )?;

    watcher.watch(watch_path.as_ref(), RecursiveMode::Recursive)?;

    'event: for e in rx {

        match e {
            Ok(event) => {
                match event.kind {
                    // We are using a poller, no check for folders or file types
                    EventKind::Create(CreateKind::Any) => {
                        
                        let (path, is_dir) = match event.paths.first() {
                            Some(path) => {                             
                                (path.to_owned(), path.is_dir())
                            },
                            None => {
                                log::error!("[{path_name}] Could not extract input path from event");
                                continue 'event;
                            }
                        };

                        if is_dir {

                            // Check that the created directory is
                            // one level down from the watch path
                            // otherwise any other subdirectories 
                            // will be captured
                            match path.parent() {
                                Some(parent_dir) => {
                                    if parent_dir != watch_path.as_ref() {
                                        continue 'event;
                                    }
                                },
                                None => {
                                    log::error!("[{path_name}] Could not extract parent directory from event");
                                    continue 'event;
                                }
                            }

                            // Spawn a thread that handles input directory completion 
                            // and validation to launch the workflow run 

                            thread::spawn(move || {
                                log::info!("[{path_name}] Input directory detected: {}", path.display());

                                match watch_event_timeout(path.clone(), timeout_interval, timeout) {
                                    Ok(_) => {
                                        
                                        match WorkflowLauncher::new(
                                            PathBuf::new(), path  // TODO BASE PATH
                                        ) {
                                            Ok(launcher) => {
                                                
                                                if let Err(err) = launcher.validate_inputs() {
                                                    log::warn!("[{path_name}] Failed to validate inputs (error: {})", err.to_string())
                                                }

                                                // Launch workflows for each x files observed by watcher e.g. every 10 Blow5 outputs
                                                
                                            }, 
                                            Err(err) => {
                                                log::warn!("[{path_name}] Could not initiate workflow launcher (error: {})", err.to_string())
                                            }
                                        };
                                    }, 
                                    Err(err) => {
                                        log::warn!("[{path_name}] Failed to watch input directory for completion (error: {})", err.to_string());
                                    }
                                };
                                
                            });
                        }

                        // Not joining thread - if the main watch process fails, threads will exit!

                    },
                    _ => {}
                }
            },
            Err(_) => {}
        }
    }

    Ok(())
}


pub fn watch_event_timeout<P: AsRef<Path>>(path: P, interval: Duration, timeout: Duration) -> notify::Result<()> {

    let path_name = match path.as_ref().file_name() {
        Some(name) => name.to_str().unwrap_or("unknown"), None => "unknown"
    };
    
    log::info!("[{path_name}] Watching input directory for changes...");

    let (tx, rx) = std::sync::mpsc::channel();

    let tx_c = tx.clone();
    
    let mut watcher = PollWatcher::new(
        move |watch_event| {
            tx_c.send(watch_event).unwrap();
        },
        Config::default().with_poll_interval(interval)
    )?;

    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    loop {
        match rx.recv_timeout(timeout) {
            Ok(event) => {
                log::info!("[{path_name}] Event received, continue polling...");

                // If the event is an error (e.g. timeout directory deleted)
                // this handle will terminate the timeout watcher 
                if let Err(err) = event {
                    log::warn!("[{path_name}] Error in poll watcher, terminating polling");
                    return Err(err)
                }

            },
            Err(_) => {
                log::info!("[{path_name}] No event received before timeout");
                break
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    log::info!("[{path_name}] Timeout watcher thread completed");
    log::info!("[{path_name}] Continue with input checks and notifications");

    Ok(())
}