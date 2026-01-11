# LifeInOrderBackend

A Rust backend for a habit tracking application built with Actix Web and PostgreSQL.

## Requirements

- Rust (edition 2024)
- PostgreSQL

## Setup

1. Create a PostgreSQL database:
   ```sql
   CREATE DATABASE life_in_order;
   ```

2. Create a `.env` file:
   ```env
   DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/life_in_order
   HOST=0.0.0.0
   PORT=8080
   ```

3. Run the server:
   ```bash
   cargo run
   ```

Migrations run automatically on startup.

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/users` | Create user |
| GET | `/users/{user_id}/config` | Get user's habits |
| GET | `/users/{user_id}/list` | Get habit values (supports zoom levels) |
| POST | `/user_habits` | Create habit |
| PUT | `/user_habits` | Update habit |
| DELETE | `/user_habits/{id}` | Delete habit |
| POST | `/user_habits/reorder` | Reorder habits |
| POST | `/habit_values` | Create habit value |
| PUT | `/habit_values` | Update habit value |
| DELETE | `/habit_values/{id}` | Delete habit value |
| POST | `/habit_values/reorder` | Reorder habit values |
| POST | `/day_values` | Record daily entry |
