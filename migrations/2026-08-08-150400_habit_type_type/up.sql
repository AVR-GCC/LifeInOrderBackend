CREATE TYPE habit_type AS ENUM ('color', 'text', 'number');

ALTER TABLE user_habits ALTER COLUMN habit_type DROP DEFAULT;

ALTER TABLE user_habits
  ALTER COLUMN habit_type TYPE habit_type
  USING habit_type::habit_type;

ALTER TABLE user_habits ALTER COLUMN habit_type SET DEFAULT 'color'::habit_type;
