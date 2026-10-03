use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::{Mutex, PoisonError};
use std::thread;

/// How many jobs may be dispatched or waiting to be emitted, per worker.
///
/// A window of exactly one per worker stalls the pool behind one slow file: the
/// other workers finish, nothing new can be dispatched until the slow one is
/// emitted, and they sit idle. Two per worker lets them keep going for a while.
/// The window is also the most results ever held in memory at once, so it must
/// stay a small multiple of the worker count, never a function of the job count.
const WINDOW_PER_WORKER: usize = 2;

/// One job and the slot its result goes back through.
type Job<T, R> = (T, mpsc::Sender<thread::Result<R>>);

/// Runs `work` on every job across `workers` threads, and calls `emit` on the
/// results in job order.
///
/// Jobs are pulled from `jobs` lazily, and no more than
/// `workers * WINDOW_PER_WORKER` are ever dispatched but not yet emitted. So the
/// thread count is fixed whatever the job count, and so is the number of results
/// held in memory waiting for an earlier, slower job to finish.
///
/// `jobs` is consumed on the calling thread, interleaved with `emit`. A slow
/// iterator, such as a directory walk, is therefore overlapped with the search
/// rather than run to completion before it.
///
/// A panic in `work` is carried back and resumed on the calling thread at that
/// job's position, after the workers have shut down. It never leaves a slot
/// empty, which is what would otherwise make the caller wait forever.
pub fn run_ordered<T, R>(
    jobs: impl IntoIterator<Item = T>,
    workers: usize,
    work: impl Fn(T) -> R + Sync,
    mut emit: impl FnMut(R),
) where
    T: Send,
    R: Send,
{
    let workers = workers.max(1);
    let window = workers * WINDOW_PER_WORKER;

    let (job_tx, job_rx) = mpsc::channel::<Job<T, R>>();
    let job_rx = Mutex::new(job_rx);

    thread::scope(|scope| {
        // Moved in, not borrowed, so that a panic resumed below drops it on the
        // way out. The workers then see the channel close and the scope can
        // join them; with the sender still alive it would wait forever.
        let job_tx = job_tx;

        for _ in 0..workers {
            let job_rx = &job_rx;
            let work = &work;
            scope.spawn(move || loop {
                // The guard is a temporary of this statement, so the lock is
                // released before the job runs, not after.
                let job = job_rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
                let Ok((job, result_tx)) = job else {
                    // The sender is gone: no more jobs.
                    return;
                };
                let result = panic::catch_unwind(AssertUnwindSafe(|| work(job)));
                // The receiver is gone only if the caller is already unwinding.
                let _ = result_tx.send(result);
            });
        }

        // One receiver per dispatched job, oldest first. Emitting strictly from
        // the front is what keeps the output in job order.
        let mut pending: VecDeque<mpsc::Receiver<thread::Result<R>>> = VecDeque::new();
        let mut emit_front = |pending: &mut VecDeque<mpsc::Receiver<_>>| {
            let rx = pending.pop_front().expect("only called with a job pending");
            match rx.recv() {
                Ok(Ok(result)) => emit(result),
                Ok(Err(payload)) => panic::resume_unwind(payload),
                // Every worker catches its job's panic and sends, so a slot
                // cannot be dropped unfilled.
                Err(mpsc::RecvError) => unreachable!("a worker dropped a result slot"),
            }
        };

        for job in jobs {
            if pending.len() >= window {
                emit_front(&mut pending);
            }
            let (result_tx, result_rx) = mpsc::channel();
            job_tx
                .send((job, result_tx))
                .expect("workers outlive the job sender");
            pending.push_back(result_rx);
        }

        // Without this the workers never see the channel close, and the scope
        // waits on them forever.
        drop(job_tx);

        while !pending.is_empty() {
            emit_front(&mut pending);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn results_come_out_in_job_order_whatever_order_they_finish_in() {
        let mut out = Vec::new();
        // Earlier jobs sleep longer, so they finish last.
        run_ordered(
            0..20u64,
            4,
            |i| {
                thread::sleep(Duration::from_millis(20 - i));
                i
            },
            |i| out.push(i),
        );
        assert_eq!(out, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn never_runs_more_jobs_at_once_than_there_are_workers() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let mut emitted = 0;

        run_ordered(
            0..64,
            3,
            |_| {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(2));
                running.fetch_sub(1, Ordering::SeqCst);
            },
            |()| emitted += 1,
        );

        assert_eq!(emitted, 64);
        assert!(peak.load(Ordering::SeqCst) <= 3, "peak {peak:?}");
    }

    /// The bound that keeps memory flat: a job is never dispatched while the
    /// window ahead of the oldest unemitted job is full, however slow that job is.
    #[test]
    fn never_dispatches_further_ahead_of_the_output_than_the_window() {
        let workers = 2;
        let window = workers * WINDOW_PER_WORKER;
        let emitted = AtomicUsize::new(0);
        let furthest_ahead = AtomicUsize::new(0);

        run_ordered(
            0..50usize,
            workers,
            |i| {
                let ahead = i - emitted.load(Ordering::SeqCst);
                furthest_ahead.fetch_max(ahead, Ordering::SeqCst);
                // Job 0 holds the front of the queue, so everything behind it
                // piles up against the window.
                if i == 0 {
                    thread::sleep(Duration::from_millis(50));
                }
            },
            |()| {
                emitted.fetch_add(1, Ordering::SeqCst);
            },
        );

        assert_eq!(emitted.load(Ordering::SeqCst), 50);
        assert!(
            furthest_ahead.load(Ordering::SeqCst) < window,
            "dispatched {furthest_ahead:?} ahead, window is {window}"
        );
    }

    #[test]
    fn zero_workers_is_treated_as_one_instead_of_hanging() {
        let mut out = Vec::new();
        run_ordered(0..3, 0, |i| i, |i| out.push(i));
        assert_eq!(out, vec![0, 1, 2]);
    }

    #[test]
    fn no_jobs_emits_nothing() {
        let mut emitted = 0;
        run_ordered(std::iter::empty::<u8>(), 4, |i| i, |_| emitted += 1);
        assert_eq!(emitted, 0);
    }

    /// A panicking job must surface on the caller rather than leave it waiting
    /// on a slot that will never be filled.
    #[test]
    fn a_panic_in_a_job_reaches_the_caller_after_earlier_results() {
        let mut out = Vec::new();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            run_ordered(
                0..10,
                3,
                |i| {
                    assert!(i != 4, "job four");
                    i
                },
                |i| out.push(i),
            );
        }));
        assert!(result.is_err());
        assert_eq!(out, vec![0, 1, 2, 3]);
    }
}
