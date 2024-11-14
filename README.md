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

The server is configured for 5 threads, and maximum 5 async tasks per thread.

The connection pool size is set to 25 connections, one for each possible async task.

### The patch

The patch tries not to change much internal logic of gRPC. We replace [a single function](async_grpc_patch.rb) with an
async version.

In a nutshell, wrap each thread's main loop in Fiber scheduler provided by the [async
gem](https://github.com/socketry/async).

### Async gem

Using this gem has several advantages over raw Fibers:

1. It provides a compatible [Fiber scheduler](https://docs.ruby-lang.org/en/3.1/Fiber/SchedulerInterface.html)
2. It provides a simple abstraction over Fibers, and some useful utilities[[1]](#1)

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
| 5     | 20   | 5.92s    | 100        | 0        | 0       | 0           | 0            | 0          | 16.89              | 295/352/266    |
| 10    | 20   | 12.98s   | 200        | 0        | 30      | 15          | 0            | 0          | 15.41              | 297/337/268    |
| 20    | 20   | 26.55s   | 385        | 15       | 135     | 105         | 0            | 0          | 14.50              | 311/375/271    |
| 40    | 20   | 53.55s   | 695        | 105      | 525     | 525         | 0            | 0          | 12.98              | 314/407/269    |
| 80    | 20   | 96.96s   | 1085       | 515      | 1975    | 2275        | 0            | 0          | 11.19              | 315/418/267    |

#### Async

| Tasks | Loop | Duration | Successful | RetryMax | Retries | gRPC ResExh | gRPC Unavail | gRPC Other | Throughput (req/s) | Avg/Max/Min ms |
|-------|------|----------|------------|----------|---------|-------------|--------------|------------|--------------------|----------------|
| 5     | 20   | 5.74s    | 100        | 0        | 0       | 0           | 0            | 0          | 17.41              | 286/350/265    |
| 10    | 20   | 7.80s    | 200        | 0        | 4       | 2           | 0            | 0          | 25.63              | 295/328/257    |
| 20    | 20   | 9.09s    | 400        | 0        | 40      | 20          | 0            | 0          | 44.02              | 326/415/262    |
| 40    | 20   | 14.76s   | 800        | 0        | 302     | 151         | 0            | 0          | 54.21              | 350/738/262    |
| 80    | 20   | 25.64s   | 1586       | 14       | 1240    | 655         | 0            | 0          | 61.85              | 359/723/257    |

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

<a id=1>1.</a> Example:

### bounded

```ruby
Async do
  semaphore = Async::Semaphore.new(2)
  (1..5).each do |i|
    semaphore.async do
      puts "Enter task #{i}"
      sleep 1
      puts "Exit task #{i}"
    end
  end
end

```

Output

```
Enter task 1
Enter task 2
Exit task 1
Enter task 3
Exit task 2
Enter task 4
Exit task 3
Enter task 5
Exit task 4
Exit task 5
```

### unbounded

```ruby
Async do |task|
  (1..5).each do |i|
    task.async do
      puts "Enter task #{i} fiber=#{Fiber.current}"
      sleep 1
      puts "Exit task #{i}"
    end
  end
end
```

Output

```
Enter task 1
Enter task 2
Enter task 3
Enter task 4
Enter task 5
Exit task 1
Exit task 2
Exit task 3
Exit task 4
Exit task 5
```
