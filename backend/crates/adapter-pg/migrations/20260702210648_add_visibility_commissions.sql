-- Add migration script here
ALTER TABLE commission
ADD COLUMN visibility text NOT NULL;

