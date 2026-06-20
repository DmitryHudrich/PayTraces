CREATE TABLE chains (
    id                   INT         PRIMARY KEY,
    name                 TEXT        NOT NULL,
    family               TEXT        NOT NULL CHECK (family IN ('evm', 'tron', 'bitcoin', 'solana', 'other')),
    address_model        TEXT        NOT NULL CHECK (address_model IN ('account', 'utxo')),
    address_encoding     TEXT        NOT NULL CHECK (address_encoding IN ('hex20', 'tron_base58_check', 'bech32', 'base58')),
    native_asset_symbol  TEXT        NOT NULL,
    native_asset_decimals SMALLINT   NOT NULL,
    confirmation_depth   INT         NOT NULL DEFAULT 12,
    enabled              BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO chains
    (id, name, family, address_model, address_encoding, native_asset_symbol, native_asset_decimals, confirmation_depth)
VALUES
    (  1, 'Ethereum', 'evm',     'account', 'hex20',             'ETH',  18, 12),
    (195, 'Tron',     'tron',    'account', 'tron_base58_check', 'TRX',   6, 20),
    (  0, 'Bitcoin',  'bitcoin', 'utxo',    'bech32',            'BTC',   8,  6),
    (501, 'Solana',   'solana',  'account', 'base58',            'SOL',   9, 32)
ON CONFLICT (id) DO NOTHING;

ALTER TABLE transfers
    ADD CONSTRAINT transfers_chain_fk
    FOREIGN KEY (chain_id) REFERENCES chains(id) ON DELETE RESTRICT;

ALTER TABLE entity_addresses
    ADD CONSTRAINT entity_addresses_chain_fk
    FOREIGN KEY (chain_id) REFERENCES chains(id) ON DELETE RESTRICT;
