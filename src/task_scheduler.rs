use std::{
    thread,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use threadpool::ThreadPool;


pub type FnTask = Fn() -> Duration + Send + 'static;


pub struct Task {
    job: Arc<Mutex<FnTask>>,
    next_time: Instant,
    running: bool,
}

impl Task {
    pub fn new<F>(job: F, period: Duration) -> Self where
        F: Fn() -> Duration + Send + 'static {
            
        Task {
            job: Arc::new(Mutex::new(job)),
            next_time: Instant::now() + period,
            running: false,
        }
    }

    fn ready(&self) -> bool {
        !self.running && Instant::now() >= self.next_time
    }

    fn finish_job(&mut self, result: Duration) {
        self.next_time = Instant::now() + result;
        self.running = false;
    }

    fn mark_as_busy(&mut self) {
        self.running = true;
    }

    fn get_job(&self) -> Arc<Mutex<FnTask>> {
        self.job.clone()
    }
}


pub struct TaskSchedulerBuilder {
    tasks: Vec<Arc<Mutex<Task>>>,
    n_workers: usize,
    period_resolution: Duration,
}

impl TaskSchedulerBuilder {
    pub fn new() -> Self {
        TaskSchedulerBuilder {
            tasks: Vec::new(),
            n_workers: 4,
            period_resolution: Duration::new(1, 0),
        }
    }

    pub fn n_workers(mut self, cnt: usize) -> Self {
        self.n_workers = cnt;
        self
    }

    pub fn period_resolution(mut self, period: Duration) -> Self {
        self.period_resolution = period;
        self
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(Arc::new(Mutex::new(task)));
    }

    pub fn build(self) -> TaskScheduler {
        TaskScheduler::new(self)
    }
}


pub struct TaskScheduler {
    scheduler: thread::JoinHandle<()>,
    running: Arc<Mutex<bool>>,
}

impl TaskScheduler {
    fn new(builder: TaskSchedulerBuilder) -> Self {
        let mut tasks = builder.tasks;
        let n_workers = builder.n_workers;
        let period_resolution = builder.period_resolution;

        let t_running = Arc::new(Mutex::new(true));
        let running = t_running.clone();

        let scheduler_job = move || {
            let pool = ThreadPool::new(n_workers);

            while *running.lock().unwrap() {
                for m_task in &mut tasks {
                    let mut task = m_task.lock().unwrap();
                    
                    if task.ready() {
                        task.mark_as_busy();

                        let job = task.get_job();
                        let t_task = m_task.clone();
                        pool.execute(move || {
                            let next_period = (*job.lock().unwrap())();
                            t_task.lock().unwrap().finish_job(next_period);
                        });
                    }
                }

                thread::sleep(period_resolution);
            }

            pool.join();
        };

        TaskScheduler {
            scheduler: thread::spawn(scheduler_job),
            running: t_running,
        }
    }

    pub fn join(self) {
        {
            let mut running = self.running.lock().unwrap();

            if !*running {
                return;
            }

            *running = false;
        }

        self.scheduler.join().unwrap();
    }
}