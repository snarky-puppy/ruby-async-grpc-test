# frozen_string_literal: true

$LOAD_PATH.unshift(File.expand_path(__dir__))

require 'test_service/v1/v1_services_pb'
require 'config'

if ARGV.find { |x| x  == '--async' }
  puts "ENABLE ASYNC PATCH"
  require 'async_grpc_patch'
else
  puts "ASYNC PATCH NOT ENABLED"
end

def fib(f)
  f <= 1 ? f : fib(f - 1) + fib(f - 2)
end

class MyTestService < TestService::V1::TestService::Service
  def cpu(_request, _call)
    response = TestService::V1::CpuResponse.new(
      fib: fib(30)
    )
    response
  end

  def db(_request, _call)
    ActiveRecord::Base.connection_pool.with_connection do
      (1..10).each do
        User.create(email: Digest::SHA256.hexdigest(Random.random_bytes(10)))
      end
      count = User.count_by_sql('SELECT COUNT(*), pg_sleep(0.25) FROM users')
      TestService::V1::DbResponse.new(
        result: count.to_s
      )
    end
  end

end

def start_test_service(pool_size: 5)
  # spin up test grpc server
  server = GRPC::RpcServer.new(pool_size: pool_size)
  server.add_http2_port('0.0.0.0:50053', :this_port_is_insecure)
  server.handle(MyTestService)

  Thread.new {
    server.run_till_terminated
  }

  sleep 1

  server
end

if __FILE__ == $PROGRAM_NAME
  svc = start_test_service

  puts "Type q<ENTER> to stop"

  loop do
    input = $stdin.gets # Wait for user input

    # Exit gracefully, this lets us gather memory usage stats
    # Also the C library seems to segfault on exit...
    case input
    when "q\n"
      break
    when nil
      break
    end
  end

  svc.stop
end
