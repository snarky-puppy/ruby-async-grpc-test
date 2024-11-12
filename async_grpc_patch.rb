# frozen_string_literal: true

require 'async'
require 'async/semaphore'

MAX_TASKS_PER_THREAD = 5

=begin original:

    def loop_execute_jobs(worker_queue)
      loop do
        begin
          blk, args = worker_queue.pop
          blk.call(*args)
        rescue StandardError, GRPC::Core::CallError => e
          GRPC.logger.warn('Error in worker thread')
          GRPC.logger.warn(e)
        end
        # there shouldn't be any work given to this thread while its busy
        fail('received a task while busy') unless worker_queue.empty?
        @stop_mutex.synchronize do
          return if @stopped
          @ready_workers << worker_queue
        end
      end
    end
=end
module AsyncGRPCPoolExtension

  def loop_execute_jobs(worker_queue)
    ::Async::Reactor.run do |task|
      semaphore = Async::Semaphore.new(MAX_TASKS_PER_THREAD)
      loop do
        blk, args = worker_queue.pop

        semaphore.async do
          # { throw :exit } is queued by Pool#stop
          catch(:exit) do
            blk.call(*args)
          end
        rescue StandardError, GRPC::Core::CallError => e
          GRPC.logger.warn('Error in worker thread')
          GRPC.logger.warn(e)
        end

        # the async task will run until the first blocking call, then it should `yield` so that
        # execution will arrive here.

        @stop_mutex.synchronize do
          if @stopped
            # make sure to clean up the parent task
            task.stop
            return
          end
          # push our queue back onto @ready_workers so we can accept more work
          @ready_workers << worker_queue
        end
      end
    end
  end
end

GRPC::Pool.prepend(AsyncGRPCPoolExtension)
