# frozen_string_literal: true

require 'grpc'
require 'active_record'
require 'yaml'
require 'erb'

# Load the database configuration
DB_CONFIG = YAML.load(ERB.new(File.read('db/config.yml')).result)

# Establish the database connection
ActiveRecord::Base.establish_connection(DB_CONFIG['test'])

# This means that each Fiber gets its own connection, isolated from other Fibers.
# The default is "thead isolation" which means the same thing, but for threads.
ActiveSupport::IsolatedExecutionState.isolation_level = :fiber

# ActiveRecord::Base.logger = Logger.new(STDOUT)

# Define the User model
class User < ActiveRecord::Base
end

module GRPC
  def self.logger
    @logger ||= Logger.new(STDOUT)
  end
end
