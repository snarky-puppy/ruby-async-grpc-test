# frozen_string_literal: true

require_relative '../config'
require 'async'

describe 'Async active record' do

  before(:all) do
    User.create(email: 'matt')
  end

  after(:all) do
    User.delete_all
  end

  it 'retrieves a user' do
    user = User.first
    expect(user.email).to eq('matt')
  end

  it 'ensures our 2 database connections are shared fairly between async tasks' do
=begin
Connection must be returned to the pool after use with ActiveRecord::Base.clear_active_connections!
Otherwise, the connection will be held until past the end of the task.

AR does NOT automatically return the connection to the pool after the task completes.

Configuring reaping_frequency in database.yml to 1 second I /could/ get this test to work without
explicitly calling clear_active_connections().

IIUC, the :thread isolation level achieves this with some kind of request scope gem hook. So it's not built into AR.(?)

Thoughts: It would be ideal from a DB connection management POV if we could not leave DB connections open
while we wander off calling APIs etc. I think we might be a ways off from implementing this pattern.
Meanwhile, for async to work we will need to allocate 1 connection per task per thread.
=end
    connection_pool = ActiveRecord::Base.connection_pool

    Async do
      tasks = 5.times.map do |i|
        Async do
          puts "Task #{i} started"
          connection = ActiveRecord::Base.connection
          connection_id = connection.object_id
          puts "Task #{i} using connection #{connection_id}"

          # Simulate some database operation
          User.create(email: "user#{i}@example.com")
          user_count = User.count

          puts "Task #{i} completed, user_count: #{user_count}"
        ensure
          # Ensure the connection is returned to the pool
          # Without this, the connection will be held until the end of the test
          # because AR does not automatically return the connection to the pool
          ActiveRecord::Base.clear_active_connections!
        end
      end
      tasks.each(&:wait)
    end

    # After all tasks complete
    connections_in_use = connection_pool.connections.size
    puts "Connections in use after tasks: #{connections_in_use}"
    expect(connections_in_use).to be <= connection_pool.size
  end

end