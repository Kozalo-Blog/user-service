CREATE TABLE IF NOT EXISTS Users (
    id bigserial PRIMARY KEY,
    name varchar(256),
    language_code char(2),
    location float8[2],
    premium_till timestamptz
);

DO $$ BEGIN
    CREATE TYPE service_type AS ENUM (
        'telegram-bot',
        'telegram-channel',
        'website',
        'application'
        );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS Services (
    id serial PRIMARY KEY,
    name varchar(64) NOT NULL,
    type service_type NOT NULL
);

CREATE TABLE IF NOT EXISTS Consents (
    uid bigint PRIMARY KEY REFERENCES Users(id),
    obtained_at timestamptz NOT NULL DEFAULT current_timestamp,
    service_id int NOT NULL REFERENCES Services(id),
    info jsonb
);

CREATE TABLE IF NOT EXISTS User_Service_Mappings (
    user_id bigint REFERENCES Users(id),
    service_id int REFERENCES Services(id),
    external_id bigint,

    PRIMARY KEY (user_id, service_id, external_id)
);
