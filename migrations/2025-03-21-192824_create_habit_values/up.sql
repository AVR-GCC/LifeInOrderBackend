CREATE TABLE habit_values (
    id SERIAL PRIMARY KEY,
    habit_id INTEGER NOT NULL REFERENCES user_habits(id) ON DELETE CASCADE,
    color VARCHAR,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);
