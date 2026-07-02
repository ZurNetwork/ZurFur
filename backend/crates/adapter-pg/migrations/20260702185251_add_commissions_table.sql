-- Add migration script here

CREATE TABLE commission (
  id uuid PRIMARY KEY,
  title text NOT NULL,
  owner_id uuid NOT NULL REFERENCES users (id),
  lifecycle text NOT NULL,
  deadline timestamptz,
  created_at timestamptz NOT NULL
)

