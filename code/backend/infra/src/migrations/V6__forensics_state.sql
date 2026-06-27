CREATE TABLE watchlist (
    chain_id   INT         NOT NULL,
    address    BYTEA       NOT NULL,
    reason     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (chain_id, address)
);

CREATE TABLE alerts (
    id         BIGSERIAL   PRIMARY KEY,
    chain_id   INT         NOT NULL,
    address    BYTEA       NOT NULL,
    tx_hash    BYTEA       NOT NULL,
    tx_idx     INT         NOT NULL,
    reason     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX alerts_address_idx ON alerts (chain_id, address, created_at DESC);

CREATE TABLE address_kind (
    chain_id     INT  NOT NULL,
    address      BYTEA NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('eoa', 'contract', 'known_service', 'unknown')),
    service_name TEXT,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (chain_id, address)
);
