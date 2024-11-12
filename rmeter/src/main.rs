mod opts;
mod test;

pub mod test_service {
    tonic::include_proto!("test_service.v1");
}

use opts::Config;
use futures::future::join_all;
use test_service::test_service_client::TestServiceClient;
use test_service::{CpuRequest, DbRequest};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

// Define the Operation enum for CPU and DB requests
#[derive(Clone, Copy)]
enum Operation {
    Cpu(CpuRequest),
    Db(DbRequest),
}

// Struct to collect and manage statistics
#[derive(Debug, Default)]
struct Stats {
    total_success: usize,
    total_failures: usize,
    total_retries: usize,
    total_resource_exhausted: usize,
    total_unavailable: usize,
    total_errors: usize,
    total_response_time: u128, // in microseconds
    max_response_time: u128,   // in microseconds
    min_response_time: u128,   // in microseconds
}

impl Stats {
    fn new() -> Self {
        Self {
            total_success: 0,
            total_failures: 0,
            total_retries: 0,
            total_resource_exhausted: 0,
            total_unavailable: 0,
            total_errors: 0,
            total_response_time: 0,
            max_response_time: 0,
            min_response_time: u128::MAX,
        }
    }

    fn update(&mut self, response_time: u128, retries: usize) {
        self.total_success += 1;
        self.total_retries += retries;
        self.total_response_time += response_time;
        if response_time > self.max_response_time {
            self.max_response_time = response_time;
        }
        if response_time < self.min_response_time {
            self.min_response_time = response_time;
        }
    }

    fn record_failure(&mut self) {
        self.total_failures += 1;
    }

    fn record_resource_exhausted(&mut self) {
        self.total_resource_exhausted += 1;
    }

    fn record_unavailable(&mut self) {
        self.total_unavailable += 1;
    }

    fn record_error(&mut self) {
        self.total_errors += 1;
    }
}

#[tokio::main]
async fn main() {
    // Parse configuration from command-line arguments or other sources
    let config = Config::from_args();

    // Display configuration parameters
    // println!("=== Configuration ===");
    // println!("Tasks: {}", config.tasks);
    // println!("Loop Count: {}", config.loop_count);
    // println!("Ramp-up Time: {} seconds", config.ramp_seconds);
    // println!("API: {}", config.api);
    // println!("Max Retries: {}", config.max_retries);
    // println!("Retry Initial Delay: {} ms", config.retry_initial_delay_ms);
    // println!("======================\n");

    // Determine the operation type based on the API specified
    let work = match config.api.as_str() {
        "cpu" => Operation::Cpu(CpuRequest {}),
        "db" => Operation::Db(DbRequest {}),
        _ => panic!("Invalid API: {}", config.api),
    };

    // Initialize shared statistics
    let stats = Arc::new(Mutex::new(Stats::new()));

    // Record the start time for throughput calculation
    let start_time = Instant::now();

    let mut tasks = Vec::new();

    // Calculate the interval between starting each task
    // Avoid division by zero by ensuring config.tasks > 0
    let interval = if config.tasks > 0 {
        config.ramp_seconds as f64 / config.tasks as f64
    } else {
        0.0
    };

    for conn_id in 0..config.tasks {
        let config2 = config.clone(); // Clone config into config2
        let work2 = work.clone();     // Clone work into work2
        let stats2 = Arc::clone(&stats); // Clone the Arc to share with the task

        // Spawn each task asynchronously
        let handle = tokio::spawn(async move {
            generic_test(config2, conn_id, work2, stats2).await;
        });
        tasks.push(handle);

        // Introduce a delay before spawning the next task to implement ramp-up
        if conn_id < config.tasks - 1 && interval > 0.0 {
            sleep(Duration::from_secs_f64(interval)).await;
        }
    }

    // Wait for all tasks to complete
    join_all(tasks).await;

    // Calculate the total duration
    let duration = start_time.elapsed();
    let duration_secs = duration.as_secs_f64();

    // Aggregate statistics
    let stats = stats.lock().await;
    let total_success = stats.total_success;
    let total_failures = stats.total_failures;
    let total_retries = stats.total_retries;
    let total_response_time = stats.total_response_time;
    let max_response_time = stats.max_response_time;
    let min_response_time = if stats.min_response_time == u128::MAX {
        0
    } else {
        stats.min_response_time
    };

    // Calculate throughput: successful requests per second
    let throughput = if duration_secs > 0.0 {
        total_success as f64 / duration_secs
    } else {
        0.0
    };

    // Calculate average response time
    let average_response_time = if total_success > 0 {
        total_response_time as f64 / total_success as f64
    } else {
        0.0
    };

    // Display the final report as a Markdown table with metrics as columns
    println!("| Tasks | Loop | Duration | Successful | RetryMax   | Retries | gRPC ResExh | gRPC Unavail | gRPC Other   | Throughput (req/s) | Avg/Max/Min ms  |");
    println!("|-------|------|----------|------------|------------|---------|-------------|--------------|--------------|--------------------|-----------------|");
    println!("| {}    | {}   | {:.2}s   | {}         | {}         | {}      | {}          | {}           | {}           | {:.2}              | {}/{}/{}        |",
        config.tasks,
        config.loop_count,
        duration_secs,
        total_success,
        total_failures,
        total_retries,
        stats.total_resource_exhausted,
        stats.total_unavailable,
        stats.total_errors,
        throughput,
        average_response_time as u32,
        max_response_time as u32,
        min_response_time as u32
    );
}

// Function to perform generic testing for each task
async fn generic_test(
    config: Config,
    conn_id: usize,
    work: Operation,
    stats: Arc<Mutex<Stats>>,
) {
    // Attempt to connect to the TestService
    let mut client = match TestServiceClient::connect("http://[::1]:50053").await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Connection error for task {}: {:?}", conn_id, e);
            let mut stats = stats.lock().await;
            stats.record_failure();
            return;
        }
    };

    // Perform the specified number of loops
    for _ in 0..config.loop_count {
        let mut retries = 0;
        let mut delay = config.retry_initial_delay_ms; // Initial retry delay in milliseconds

        loop {
            let start = Instant::now();
            let result = execute_operation(work.clone(), &mut client).await;
            let elapsed = start.elapsed().as_millis();

            match result {
                Ok(_) => {
                    // On success, update statistics and break the retry loop
                    let mut stats = stats.lock().await;
                    stats.update(elapsed, retries);
                    break;
                }
                Err(e) => {
                    retries += 1;

                    // Check if the error is ResourceExhausted
                    match e.code() {
                        tonic::Code::ResourceExhausted => {
                            let mut stats = stats.lock().await;
                            stats.record_resource_exhausted();
                        }
                        tonic::Code::Unavailable => {
                            let mut stats = stats.lock().await;
                            stats.record_unavailable();
                        }
                        _ => {
                            let mut stats = stats.lock().await;
                            stats.record_error();

                            println!(
                                "Task {}: failed: {:?}",
                                conn_id,
                                e
                            );
                            break;
                        }
                    }

                    if retries > config.max_retries {
                        // Record failure
                        let mut stats = stats.lock().await;
                        stats.record_failure();

                        break;
                    } else {
                        // Record retry attempt
                        {
                            let mut stats = stats.lock().await;
                            stats.total_retries += 1;
                        }

                        // Wait for the specified delay before retrying
                        sleep(Duration::from_millis(delay)).await;
                        // Implement exponential backoff
                        delay *= 2;
                    }
                }
            }
        }
    }
}

// Function to execute the specified operation using the TestServiceClient
async fn execute_operation(
    operation: Operation,
    client: &mut TestServiceClient<tonic::transport::Channel>,
) -> Result<(), tonic::Status> {
    match operation {
        Operation::Cpu(req) => {
            let response = client.cpu(tonic::Request::new(req)).await?;
            let _cpu_response = response.into_inner();
            // Optionally process _cpu_response
        }
        Operation::Db(req) => {
            let response = client.db(tonic::Request::new(req)).await?;
            let _db_response = response.into_inner();
            // Optionally process _db_response
        }
    }
    Ok(())
}
