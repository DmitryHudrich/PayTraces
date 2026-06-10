CREATE TABLE transfers (
    chain_id       INT         NOT NULL,
    tx_hash        BYTEA       NOT NULL,
    idx            INT         NOT NULL,

    from_addr      BYTEA       NOT NULL,
    to_addr        BYTEA       NOT NULL,

    asset_contract BYTEA,

    amount         NUMERIC     NOT NULL,
    decimals       SMALLINT    NOT NULL,

    block_height   BIGINT      NOT NULL,
    block_hash     BYTEA       NOT NULL,
    ts             TIMESTAMPTZ NOT NULL,

    kind           TEXT        NOT NULL,
    token_standard TEXT,
    vin_idx        INT,
    vout_idx       INT,
    finality       TEXT        NOT NULL,

    PRIMARY KEY (chain_id, tx_hash, idx)
);

CREATE INDEX idx_transfers_from  ON transfers (chain_id, from_addr,  block_height DESC);
CREATE INDEX idx_transfers_to    ON transfers (chain_id, to_addr,    block_height DESC);
CREATE INDEX idx_transfers_block ON transfers (chain_id, block_height);
