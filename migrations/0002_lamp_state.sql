-- Last switching state of the UV lamps, used to keep the re-strike pause across restarts.
-- changed_at is UTC (RFC 3339) so daylight saving time changes don't shift the pause.
CREATE TABLE lamp_state (
    relay      TEXT    PRIMARY KEY,
    is_on      INTEGER NOT NULL,
    changed_at TEXT    NOT NULL
);
