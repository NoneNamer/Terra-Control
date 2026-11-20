-- Initial schema. Times are local "HH:MM", timestamps local "YYYY-MM-DD HH:MM:SS".

-- One row per ISO calendar week. NULL on/off pair = load off all day (e.g. brumation).
-- Weeks without a row use [schedule_default] from config.toml.
CREATE TABLE schedule (
    week     INTEGER PRIMARY KEY CHECK (week BETWEEN 1 AND 53),
    uv1_on   TEXT,
    uv1_off  TEXT,
    uv2_on   TEXT,
    uv2_off  TEXT,
    heat_on  TEXT,
    heat_off TEXT,
    led_on   TEXT,
    led_off  TEXT,
    led_r    INTEGER NOT NULL CHECK (led_r BETWEEN 0 AND 255),
    led_g    INTEGER NOT NULL CHECK (led_g BETWEEN 0 AND 255),
    led_b    INTEGER NOT NULL CHECK (led_b BETWEEN 0 AND 255),
    led_ww   INTEGER NOT NULL CHECK (led_ww BETWEEN 0 AND 255),
    led_cw   INTEGER NOT NULL CHECK (led_cw BETWEEN 0 AND 255)
);

-- Sensor values plus the relay states at the time of the measurement.
CREATE TABLE readings (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp      TEXT    NOT NULL,
    basking_temp   REAL,
    cool_temp      REAL,
    enclosure_temp REAL,
    air_temp       REAL,
    humidity       REAL,
    uv1_on         INTEGER NOT NULL,
    uv2_on         INTEGER NOT NULL,
    heat_on        INTEGER NOT NULL,
    overheat       INTEGER NOT NULL
);
CREATE INDEX readings_timestamp ON readings (timestamp);

CREATE TABLE logs (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    level     TEXT NOT NULL,
    message   TEXT NOT NULL
);
CREATE INDEX logs_timestamp ON logs (timestamp);

-- Single row (id = 1), created from config.toml on first start.
CREATE TABLE led_settings (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    enabled       INTEGER NOT NULL,
    mode          TEXT    NOT NULL CHECK (mode IN ('natural', 'manual')),
    r             INTEGER NOT NULL,
    g             INTEGER NOT NULL,
    b             INTEGER NOT NULL,
    ww            INTEGER NOT NULL,
    cw            INTEGER NOT NULL,
    brightness    INTEGER NOT NULL CHECK (brightness BETWEEN 0 AND 100),
    season_weight REAL    NOT NULL CHECK (season_weight BETWEEN 0.0 AND 1.0)
);

-- Colour presets of the daily curve, created from config.toml on first start.
CREATE TABLE led_presets (
    name TEXT PRIMARY KEY CHECK (name IN ('morning', 'noon', 'evening')),
    r    INTEGER NOT NULL,
    g    INTEGER NOT NULL,
    b    INTEGER NOT NULL,
    ww   INTEGER NOT NULL,
    cw   INTEGER NOT NULL
);
