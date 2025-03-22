CREATE TABLE user_habits (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR NOT NULL,
    weight INTEGER NOT NULL,
    type VARCHAR NOT NULL DEFAULT 'color',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_type CHECK (type IN ('color', 'text', 'number'))
);
