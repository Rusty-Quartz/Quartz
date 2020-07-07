use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Sender, SendError}
};
use std::thread::{self, JoinHandle};
use log::error;

/// A dynamically sized, load-spreading threadpool that executes a specific function. The function this
/// pool's workers execute accepts an owned "job" and a mutable reference to an internal "state". This
/// model was adopted as oppsed to closures to allow for workers to be spawned without the function/closure
/// needing to be respecified. Because of this cloneability, dynamic threadpools have a `resize` method
/// which only requires a min and max bound on the new size and a load scaling constant. The actual size
/// this pool chooses within those bounds will be based on a measured load value which is recalculated every
/// time the threadpool is resized.
pub struct DynamicThreadPool<J, S, E> {
    name: String,
    pool: Vec<Worker<J>>,
    executor: fn(J, &mut S) -> Result<(), E>,
    initial_state: S
}

impl<J, S, E> DynamicThreadPool<J, S, E> {
    /// Closes this pool and joins all underlying worker threads.
    pub fn close(&mut self) {
        for worker in self.pool.drain(..) {
            worker.join();
        }
    }
}

impl<J, S, E> DynamicThreadPool<J, S, E>
where
    J: Send + 'static,
    S: Send + Clone + 'static,
    E: Into<Box<dyn Error>> + 'static
{
    /// Creates a new threadpool with the given name and initial size. The initial state provided is the
    /// state in which all new worker threads will be spawned. This method spawns the given number of worker
    /// threads which immediately block while wait for incoming jobs to complete.
    pub fn open(name: String, initial_size: usize, initial_state: S, executor: fn(J, &mut S) -> Result<(), E>) -> Self {
        let mut pool = DynamicThreadPool {
            name,
            pool: Vec::with_capacity(initial_size.max(1)),
            executor,
            initial_state
        };

        for i in 0..pool.pool.capacity() {
            pool.add_worker(i + 1);
        }

        pool
    }

    /// Adds a worker to this pool with the given number.
    fn add_worker(&mut self, number: usize) {
        self.pool.push(Worker::spawn(self.name.clone(), number, self.initial_state.clone(), self.executor));
    }

    /// Adds a job for the pool to complete. The pool will select the worker with the minimum number of pending
    /// jobs to execute the given job.
    pub fn add_job(&mut self, job: J) {
        let mut min_pending = usize::MAX;
        let mut available_worker: Option<&Worker<J>> = None;

        for worker in self.pool.iter() {
            // Calculate the load
            let pending_jobs = worker.pending_job_count();

            // Find a worker
            if pending_jobs == 0 {
                available_worker = Some(worker);
                break;
            } else if pending_jobs < min_pending {
                min_pending = pending_jobs;
                available_worker = Some(worker);
            }
        }

        if let Some(worker) = available_worker {
            if let Err(e) = worker.send_job(job) {
                error!("Failed to send job to worker in {}: {}", self.name, e);
            }
        }
    }

    /// Resizes this threadpool based on the calculated load and the given bounds. The load is calculated as
    /// the total number of pending jobs among all workers in this pool.
    pub fn resize(&mut self, load_factor: usize, mut min: usize, max: usize) {
        let load: usize = self.pool.iter().map(|worker| worker.pending_job_count()).sum();
        // Enforce min > 0 and max >= min, and then enforce min <= size <= max
        min = min.max(1);
        let new_size = (load / load_factor).max(min).min(max.max(min));

        // Nothing needs to be changed
        if new_size == self.pool.len() {
            return;
        }
        // Some workers can be removed
        else if new_size < self.pool.len() {
            for worker in self.pool.drain(new_size..) {
                worker.join();
            }
        }
        // Some workers need to be added
        else {
            for i in self.pool.len()..new_size {
                self.add_worker(i + 1);
            }
        }
    }
}

impl<J, S, E> Drop for DynamicThreadPool<J, S, E> {
    fn drop(&mut self) {
        self.close();
    }
}

/// A worker for a dynamic threadpool. Workers keep track of their pending job count autonomously.
struct Worker<J> {
    job_sender: Sender<Option<J>>,
    pending_job_count: Arc<AtomicUsize>,
    handle: JoinHandle<()>
}

impl<J> Worker<J> {
    /// The number of pending jobs this worker has.
    fn pending_job_count(&self) -> usize {
        self.pending_job_count.load(Ordering::SeqCst)
    }

    /// Join this worker's thread.
    fn join(self) {
        // There isn't really anything useful we could do with the errors that could occur here
        drop(self.job_sender.send(None));
        drop(self.handle.join());
    }
}

impl<J: Send + 'static> Worker<J> {
    /// Spawns a new worker thread with the given parameters, returning a handle to the worker thread.
    fn spawn<S, E>(
        pool_name: String,
        number: usize,
        mut state: S,
        executor: fn(J, &mut S) -> Result<(), E>
    ) -> Worker<J>
    where
        S: Send + 'static,
        E: Into<Box<dyn Error>> + 'static
    {
        // Initialize thread variables
        let (job_sender, job_receiver) = mpsc::channel::<Option<J>>();
        let pending_job_count = Arc::new(AtomicUsize::new(0));
        let job_count_clone = pending_job_count.clone();

        // Spawn the thread
        let handle = thread::spawn(move || {
            while let Ok(Some(job)) = job_receiver.recv() {
                if let Err(e) = executor(job, &mut state) {
                    error!("Error handling job in {}/Worker#{}: {}", pool_name, number, e.into());
                }

                // We completed a job :D
                job_count_clone.fetch_sub(1, Ordering::SeqCst);
            }
        });

        Worker {
            pending_job_count,
            job_sender,
            handle
        }
    }

    /// Send a job to this worker to execute.
    fn send_job(&self, job: J) -> Result<(), SendError<Option<J>>> {
        self.pending_job_count.fetch_add(1, Ordering::SeqCst);
        self.job_sender.send(Some(job))
    }
}