use std::iter::Iterator;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::future::{self, TryJoinAll};
use rayon::iter::ParallelIterator as _;
use rayon::prelude::*;
use tokio;
use tokio::stream::{Stream, StreamExt as _};
use tokio::sync::mpsc::{self, error::SendError, Receiver};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

struct ParallelIterator<T> {
    rx: Receiver<T>,
    _handle: TryJoinAll<JoinHandle<Result<(), SendError<T>>>>,
}

impl<T> Stream for ParallelIterator<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Receiver::<T>::poll_next(Pin::new(&mut self.rx), cx)
    }
}

fn parallel_map<I, F, T, U>(iter: I, op: F, n: usize) -> ParallelIterator<U>
where
    I: 'static + Iterator<Item = T> + Send,
    F: 'static + Send + Sync + Clone + Fn(T) -> U,
    T: 'static + Send,
    U: 'static + Send,
{
    let (tx, rx) = mpsc::channel::<U>(n);
    let iter = Arc::new(Mutex::new(iter));
    let tasks = (0..n).map(|_| {
        let iter = Arc::clone(&iter);
        let mut tx = tx.clone();
        let op = op.clone();
        let task: JoinHandle<Result<(), SendError<U>>> = tokio::spawn(async move {
            loop {
                let item = {
                    match iter.lock().await.next() {
                        Some(item) => item,
                        None => return Ok(()),
                    }
                };
                tx.send(op(item)).await?;
            }
        });
        task
    });
    let handle = future::try_join_all(tasks);
    ParallelIterator {
        rx,
        _handle: handle,
    }
}

#[tokio::main(core_threads = 8)]
pub async fn run() -> crate::Result<()> {
    let inputs: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let nums: Vec<usize> = inputs
        .par_iter()
        .map(|i| {
            eprintln!("start: {}", i);
            for _ in 0..100_000_000 {}
            eprintln!("finish: {}", i);
            i * i
        })
        .collect();
    eprintln!("{:?}", nums);

    let nums: Vec<usize> = parallel_map(
        0..10,
        |i| {
            eprintln!("start: {}", i);
            for _ in 0..100_000_000 {}
            eprintln!("finish: {}", i);
            i * i
        },
        5,
    )
    .collect()
    .await;
    eprintln!("{:?}", nums);
    Ok(())
}
