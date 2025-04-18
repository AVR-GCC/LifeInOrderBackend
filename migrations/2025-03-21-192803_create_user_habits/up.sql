CREATE TABLE user_habits (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR NOT NULL,
    weight INTEGER NOT NULL,
    sequence INTEGER NOT NULL,
    habit_type VARCHAR NOT NULL DEFAULT 'color',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT valid_type CHECK (habit_type IN ('color', 'text', 'number'))
);
