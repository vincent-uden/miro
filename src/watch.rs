use std::{fs, path::PathBuf, time::Duration};

use async_watcher::{AsyncDebouncer, notify::RecursiveMode};
use iced::{
    futures::{SinkExt, Stream},
    stream,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

#[derive(Debug)]
pub enum WatchMessage {
    StartWatch(PathBuf),
    StopWatch(PathBuf),
}

#[derive(Debug, Clone)]
pub enum WatchNotification {
    Ready(mpsc::Sender<WatchMessage>),
    Changed(PathBuf),
}

pub fn file_watcher() -> impl Stream<Item = WatchNotification> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);
        let _ = output.send(WatchNotification::Ready(sender)).await;

        let (mut debouncer, mut file_events) =
            AsyncDebouncer::new_with_channel(Duration::from_secs(1), Some(Duration::from_secs(1)))
                .await
                .unwrap();

        loop {
            tokio::select! {
                Some(msg) = receiver.recv() => {
                    match msg {
                        WatchMessage::StartWatch(path_buf) => {
                            let canonical = fs::canonicalize(path_buf).unwrap();
                            debug!("Starting watch on {:?}", canonical);
                            debouncer.watcher().watch(&canonical, RecursiveMode::Recursive).unwrap()
                        }
                        WatchMessage::StopWatch(path_buf) => {
                            let canonical = fs::canonicalize(path_buf).unwrap();
                            debug!("Stopping watch on {:?}", canonical);
                            debouncer.watcher().unwatch(&canonical).unwrap()
                        }
                    }
                }
                Some(file_event) = file_events.recv() => {
                    match file_event {
                        Ok(events) => {
                            for e in &events {
                                match e.event.kind {
                                    async_watcher::notify::EventKind::Modify(_) => {
                                        debug!("File modified");
                                        let _ = output.send(WatchNotification::Changed(e.event.paths[0].clone())).await;
                                    }
                                    _ => {
                                    }
                                }
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
                else => {
                    panic!("File watcher channels closed, this shouldn't happen.");
                }
            }
        }
    })
}
