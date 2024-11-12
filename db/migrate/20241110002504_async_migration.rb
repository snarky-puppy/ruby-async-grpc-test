class AsyncMigration < ActiveRecord::Migration[7.1]
  def change
    create_table :users do |t|
      t.text :email

      t.timestamps
    end
  end
end
