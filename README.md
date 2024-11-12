test_service
============

## Intro - What are we testing?

We want to test the performance of a gRPC server using an async event loop, and compare it to a traditional threaded
execution model.

To do this we created a simple gRPC server that provides 2 APIs:

1. /cpu - a CPU bound API that calculates fib(30)
2. /db - an IO bound API that inserts some rows, selects a COUNT(*) with a delay introduced using `pg_sleep()`

It's used as a test of the async event loop patch `async_grpc_patch.rb`.

### Setup

The server can be run using `bundler exec ruby test_service.rb [--async]`. `--async` will `require` the async
patch.

### The patch

The patch tries not to change much internal logic of gRPC. We replace [a single function](async_grpc_patch.rb) with an
async version.

In a nutshell, wrap each thread's main loop in Fiber scheduler provided by the [async
gem](https://github.com/socketry/async).

### Async gem

Using this gem has several advantages over raw Fibers:

1. It provides a compatible [Fiber scheduler](https://docs.ruby-lang.org/en/3.1/Fiber/SchedulerInterface.html)
2. It provides a simple abstraction over Fibers, and some useful utilities[[1]](#1)

### Server configuration

The server is configured to use 5 threads for the thread based server and 5 async tasks per thread for the async server.

| Config   | Threads | Async Tasks |
|----------|---------|-------------|
| Threaded | 5       | 0           |
| Async    | 5       | 5           |

NB: "Threads" refers to GRPC server threads. "Async Tasks" refers to the max number of async tasks allowed per thread.

### Testing methodology

We tried using JMeter to test the server, but found it difficult to get accurate results because gRPC implements
backpressure by returning a "BUSY" error when the server is overloaded.

Instead, a simple utility `rmeter` was created to test the server.

#### Testing with `rmeter`

The 2 main parameters are `--tasks` and `--loop`.

`rmeter` implements exponential backoff. One of the issues with JMeter was that it counted backpressure responses as
errors, and counted them as throughput. `rmeter` does not count backpressure responses as errors.

Backoff retry parameters:

- initial delay: 1s
- max retries: 3

#### Test Plan

Each API (CPU bound and IO bound) was tested with increasing volume:

| Size | Number of tasks | Loop count |
|------|-----------------|------------|
| S    | 5               | 20         |
| M    | 10              | 20         |
| L    | 20              | 20         |
| XL   | 40              | 20         |

Both Async and Threaded servers were tested.

## CPU

The first test explores performance of a CPU bound API. The API calculates fib(30) on each request.

### `rmeter` Test Results

#### Threaded

| Tasks | Loop | Duration | Successful | RetryMax | Retries | gRPC ResExh | gRPC Unavail | gRPC Other | Throughput (req/s) | Avg/Max/Min ms |
|-------|------|----------|------------|----------|---------|-------------|--------------|------------|--------------------|----------------|
| 5     | 20   | 7.13s    | 100        | 0        | 0       | 0           | 0            | 0          | 14.03              | 348/392/87     |
| 10    | 20   | 14.24s   | 200        | 0        | 0       | 0           | 0            | 0          | 14.05              | 695/730/90     |
| 20    | 20   | 28.29s   | 400        | 0        | 4       | 2           | 0            | 0          | 14.14              | 1359/1491/84   |
| 40    | 20   | 56.64s   | 800        | 0        | 6       | 3           | 0            | 0          | 14.13              | 2739/3007/94   |
| 80    | 20   | 113.03s  | 1600       | 0        | 10      | 5           | 0            | 0          | 14.16              | 5481/6064/90   |

#### Async

| Tasks | Loop | Duration | Successful | RetryMax | Retries | gRPC ResExh | gRPC Unavail | gRPC Other | Throughput (req/s) | Avg/Max/Min ms |
|-------|------|----------|------------|----------|---------|-------------|--------------|------------|--------------------|----------------|
| 5     | 20   | 7.13s    | 100        | 0        | 0       | 0           | 0            | 0          | 14.03              | 348/488/84     |
| 10    | 20   | 14.07s   | 200        | 0        | 0       | 0           | 0            | 0          | 14.21              | 687/1257/87    |
| 20    | 20   | 28.38s   | 400        | 0        | 126     | 63          | 0            | 0          | 14.10              | 1133/1975/98   |
| 40    | 20   | 55.09s   | 785        | 15       | 809     | 442         | 0            | 0          | 14.25              | 1530/3481/97   |
| 80    | 20   | 110.55s  | 1569       | 31       | 1709    | 932         | 0            | 0          | 14.19              | 3087/4840/99   |

### memory_profiler results

#### Threaded

```
% bundler exec ruby-memory-profiler --no-detailed --pretty run  -- ruby ./test_service.rb
Total allocated: 76.56 MB (619711 objects)
Total retained:  9.14 MB (55552 objects)
```

#### Async

```
% bundler exec ruby-memory-profiler --no-detailed --pretty run  -- ruby ./test_service.rb --async
Total allocated: 87.13 MB (711577 objects)
Total retained:  9.23 MB (56227 objects)
```

### Results

- Throughput is similar for both async and threaded servers, as might be expected as the Fiber in a CPU bound task is
  not yielding to the event loop.
- Under heavy CPU load, the async server saw a high number of requests exceed the max retry limit. This is due to the
  CPU intentive task monopolising the event loop.
- Event loop is not a great pattern for CPU bound tasks compared to threads.

## IO (Database)

Inserts some rows, selects a COUNT(*) with a delay introduced using `pg_sleep()`

### `rmeter` Test Results

#### Threaded

| Tasks | Loop | Duration | Successful | RetryMax | Retries | gRPC ResExh | gRPC Unavail | gRPC Other | Throughput (req/s) | Avg/Max/Min ms |
|-------|------|----------|------------|----------|---------|-------------|--------------|------------|--------------------|----------------|
| 5     | 20   | 8.16s    | 100        | 0        | 10      | 5           | 0            | 0          | 12.25              | 312/404/263    |
| 10    | 20   | 16.03s   | 197        | 3        | 57      | 36          | 0            | 0          | 12.29              | 309/377/265    |
| 20    | 20   | 33.37s   | 375        | 25       | 183     | 154         | 0            | 0          | 11.24              | 321/437/260    |
| 40    | 20   | 54.85s   | 679        | 121      | 627     | 616         | 0            | 0          | 12.38              | 320/394/268    |
| 80    | 20   | 97.13s   | 1073       | 527      | 2125    | 2380        | 0            | 0          | 11.05              | 332/407/265    |

#### Async

| Tasks | Loop | Duration | Successful | RetryMax | Retries | gRPC ResExh | gRPC Unavail | gRPC Other | Throughput (req/s) | Avg/Max/Min ms |
|-------|------|----------|------------|----------|---------|-------------|--------------|------------|--------------------|----------------|
| 5     | 20   | 6.77s    | 100        | 0        | 0       | 0           | 0            | 0          | 14.77              | 337/571/280    |
| 10    | 20   | 7.30s    | 200        | 0        | 0       | 0           | 0            | 0          | 27.40              | 363/416/325    |
| 20    | 20   | 10.21s   | 400        | 0        | 34      | 17          | 0            | 0          | 39.18              | 398/729/287    |
| 40    | 20   | 18.59s   | 800        | 0        | 414     | 207         | 0            | 0          | 43.03              | 518/1034/285   |
| 80    | 20   | 38.09s   | 1566       | 34       | 1896    | 1033        | 0            | 0          | 41.12              | 561/1364/298   |

### memory_profiler results

#### Threaded

```
% bundler exec ruby-memory-profiler --no-detailed --pretty run  -- ruby ./test_service.rb
Total allocated: 419.32 MB (4691923 objects)
Total retained:  9.46 MB (56353 objects)
```

#### Async

```
% bundler exec ruby-memory-profiler --no-detailed --pretty run  -- ruby ./test_service.rb --async
Total allocated: 558.30 MB (6200899 objects)
Total retained:  9.59 MB (57025 objects)
```

### Results

- The async server has a higher throughput than the threaded server, and less pushback. This is expected since there are
  5 async tasks allowed for each thread, and the task is IO bound.

## TODO

- Analyse memory usage over time

## References

<a id=1>1.</a>Example:

```ruby
Notes
Async do
  semaphore = Async::Semaphore.new(2)
  (1..5).each do
    puts "1"
    semaphore.async do
      puts "2"
      sleep 1
      puts "3"
    end
    puts "4"
  end
end
```

Output

```
1
2
4
1
2
4
1
3
2
4
1
3
2
4
1
3
2
4
3
3
```
