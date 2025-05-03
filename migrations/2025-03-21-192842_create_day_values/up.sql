CREATE TABLE day_values (
    id SERIAL PRIMARY KEY,
    value_id INTEGER NOT NULL REFERENCES habit_values(id) ON DELETE CASCADE,
    habit_id INTEGER NOT NULL REFERENCES user_habits(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    text VARCHAR,
    number INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    UNIQUE (habit_id, date)
);
