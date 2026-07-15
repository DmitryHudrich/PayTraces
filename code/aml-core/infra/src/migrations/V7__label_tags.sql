ALTER TABLE entity_addresses ADD COLUMN attached_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE TABLE label_tags (
    tag_id        UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id     UUID        NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    category      TEXT        NOT NULL,
    label_name    TEXT,
    source        TEXT        NOT NULL,
    source_detail TEXT,
    confidence    SMALLINT    NOT NULL,
    risk_score    SMALLINT    NOT NULL DEFAULT 0,
    sanction_list TEXT,
    active        BOOLEAN     NOT NULL DEFAULT true,
    superseded_by UUID        REFERENCES label_tags(tag_id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ,
    evidence_url  TEXT
);

CREATE INDEX idx_label_tags_entity_active ON label_tags (entity_id, active);
CREATE INDEX idx_label_tags_category_active ON label_tags (category, active);

CREATE TABLE tag_history (
    event_id   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tag_id     UUID        NOT NULL,
    action     TEXT        NOT NULL,
    at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor      TEXT        NOT NULL,
    reason     TEXT
);

CREATE INDEX idx_tag_history_tag ON tag_history (tag_id);

-- Fold each existing scalar-category entity row into one legacy LabelTag so
-- nothing labelled via the old `entities.category` column is lost.
INSERT INTO label_tags (entity_id, category, label_name, source, confidence, risk_score, sanction_list, active, evidence_url)
SELECT id, category, label_name, 'legacy_import', 50, risk_score, sanction_list, true, label_url
FROM entities
WHERE category IS NOT NULL;

ALTER TABLE entities DROP COLUMN category;
ALTER TABLE entities DROP COLUMN sanction_list;
ALTER TABLE entities DROP COLUMN label_name;
ALTER TABLE entities DROP COLUMN label_url;
ALTER TABLE entities DROP COLUMN label_source;
ALTER TABLE entities DROP COLUMN risk_score;
ALTER TABLE entities ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT now();
