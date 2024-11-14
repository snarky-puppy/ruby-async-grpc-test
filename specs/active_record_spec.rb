# frozen_string_literal: true

require 'async'
require_relative '../config'

def get_pid(conn = ActiveRecord::Base.connection)
  conn.execute("SELECT pg_backend_pid()").first['pg_backend_pid']
end

describe 'Async active record' do

  before(:all) do
    # Reduce pool to 2
    ActiveRecord::Base.connection_pool.disconnect!
    ActiveRecord::Base.establish_connection(DB_CONFIG['two'])

    # Ensure the pool size is 2
    expect(ActiveRecord::Base.connection_pool.size).to eq(2)
  end

  after(:all) do
    # Reset pool size
    ActiveRecord::Base.connection_pool.disconnect!
    ActiveRecord::Base.establish_connection(DB_CONFIG['test'])
  end

  before(:each) do
    User.delete_all
    puts "rspec thread connection id=#{get_pid}"

    # NB: any DB action outside a thread or a task will check out a connection from the pool
    # and not return it.
    #
    # For example, the 'sanity check' test below, or anything in a `before` block, if run
    # before the next test, will fail the test as only 1 connection will be available in the
    # pool.
    #
    # To avoid this, we need to make sure the connection pool is cleared after each test.
    ActiveRecord::Base.connection_handler.clear_active_connections!
  end

  # NB: side effect of checking out a connection for use in rspec's thread
  it 'sanity check' do
    User.create(email: 'matt')
    user = User.first
    expect(user.email).to eq('matt')
  end

  it 'ensures 2 database connections are shared fairly between async tasks' do
    pids = []
    pids_mux = Mutex.new

    Async do |task|
      6.times.map do |i|
        task.async do
          pid = get_pid
          pids_mux.synchronize { pids << pid }
          puts "Task #{i} started, using connection #{pid}"
          ActiveRecord::Base.connection.execute("SELECT pg_sleep(0.25)")
          puts "Task #{i} completed"
        end
      end.each(&:wait)
    end

    # Expect each connection to be used equally amongst the tasks
    unique_pids = pids.uniq
    expect(unique_pids.size).to eq(2)
    expect(pids.filter { |pid| pid == unique_pids.first }.size).to eq(3)
    expect(pids.filter { |pid| pid == unique_pids.last }.size).to eq(3)

    connections_in_use = ActiveRecord::Base.connection_pool.connections.size
    puts "Connections in use after tasks: #{connections_in_use}"
    expect(connections_in_use).to be <= ActiveRecord::Base.connection_pool.size
  end

  it 'isolates transaction connections from non-transaction connections' do
    tx_conn_id = nil
    pids = []
    pids_mux = Mutex.new

    Async do |task|
      # Long running transaction
      lr_task = task.async do
        ActiveRecord::Base.transaction do
          pid = get_pid
          puts "Task LR started, using connection #{pid}"
          tx_conn_id = pid
          ActiveRecord::Base.connection.execute("SELECT pg_sleep(5)")
        end
      end

      tasks = 6.times.map do |i|
        task.async do
          pid = get_pid
          puts "Task #{i} started, using connection #{pid}"
          pids_mux.synchronize { pids << pid }
          ActiveRecord::Base.connection.execute("SELECT 1")
        end
      end

      tasks.append(lr_task).each(&:wait)
    end

    expect(pids.uniq.size).to eq(1)
    expect(tx_conn_id).not_to eq(pids.first)
  end
end
