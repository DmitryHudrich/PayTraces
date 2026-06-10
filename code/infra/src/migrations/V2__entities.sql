CREATE TABLE entities (
    id            UUID        PRIMARY KEY,
    category      TEXT        NOT NULL,
    sanction_list TEXT,
    label_name    TEXT,
    label_url     TEXT,
    label_source  TEXT,
    risk_score    SMALLINT    NOT NULL DEFAULT 0
);

CREATE TABLE entity_addresses (
    entity_id  UUID    NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    chain_id   INT     NOT NULL,
    address    TEXT    NOT NULL,
    PRIMARY KEY (chain_id, address)
);

CREATE INDEX idx_entity_addresses_entity ON entity_addresses (entity_id);
