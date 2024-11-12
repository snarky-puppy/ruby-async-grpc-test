use getopts::Options;
use std::env;
use std::process;

#[derive(Debug, Clone)]
pub struct Config {
    pub tasks: usize,
    pub loop_count: usize,
    pub ramp_seconds: u64,
    pub api: String,
    pub max_retries: usize,
    pub retry_initial_delay_ms: u64,
}

impl Config {
    pub fn from_args() -> Config {
        // Default values
        let default_tasks = 5;
        let default_loop_count = 50;
        let default_ramp = 0;
        let default_max_retries = 3;
        let default_retry_initial_delay_ms = 1000;

        // Initialize options
        let args: Vec<String> = env::args().collect();
        let program_name = &args[0];
        let mut opts = Options::new();
        opts.optopt("", "tasks", "Number of tasks (default: 5)", "TASKS");
        opts.optopt("", "loop", "Loop count per task (default: 50)", "LOOP");
        opts.optopt("", "ramp", "Ramp-up time in seconds (default: 0)", "RAMP");
        opts.optopt("", "api", "API type, either 'cpu' or 'db' (required)", "API");
        opts.optopt("", "max-retries", "Maximum number of retries (default: 3)", "RETRIES");
        opts.optopt("", "retry-ms", "Initial delay in ms before retrying (default: 1000)", "DELAY");
        opts.optflag("h", "help", "Print this help menu");

        // Parse arguments
        let matches = match opts.parse(&args[1..]) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error parsing options: {}", e);
                print_usage(&program_name, &opts);
                process::exit(1);
            }
        };

        // Check for help flag and print help if needed
        if matches.opt_present("h") {
            print_usage(&program_name, &opts);
            process::exit(0);
        }

        // Parse optional and required arguments
        let tasks = matches.opt_get_default("tasks", default_tasks).unwrap_or(default_tasks);
        let loop_count = matches.opt_get_default("loop", default_loop_count).unwrap_or(default_loop_count);
        let ramp_seconds = matches.opt_get_default("ramp", default_ramp).unwrap_or(default_ramp);
        let max_retries = matches.opt_get_default("max-retries", default_max_retries).unwrap_or(default_max_retries);
        let retry_initial_delay_ms = matches.opt_get_default("retry-ms", default_retry_initial_delay_ms).unwrap_or(default_retry_initial_delay_ms);

        // Check if the required `api` argument is provided
        let api = match matches.opt_str("api") {
            Some(api) if api == "cpu" || api == "db" => api,
            _ => {
                eprintln!("Error: --api option is required and must be either 'cpu' or 'db'");
                print_usage(&program_name, &opts);
                process::exit(1);
            }
        };

        Config {
            tasks,
            loop_count,
            ramp_seconds,
            api,
            max_retries,
            retry_initial_delay_ms,
        }
    }
}

fn print_usage(program_name: &str, opts: &Options) {
    let brief = format!("Usage: {} [options]", program_name);
    print!("{}", opts.usage(&brief));
}
