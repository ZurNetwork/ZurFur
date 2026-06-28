-- Add migration script here
ALTER TABLE account_members ADD COLUMN listed_on_profile BOOLEAN NOT NULL DEFAULT true;
