ALTER TABLE user_habits ALTER COLUMN habit_type DROP DEFAULT;

ALTER TABLE user_habits
  ALTER COLUMN habit_type TYPE text
  USING habit_type::text;

ALTER TABLE user_habits ALTER COLUMN habit_type SET DEFAULT 'color';

DROP TYPE habit_type;
