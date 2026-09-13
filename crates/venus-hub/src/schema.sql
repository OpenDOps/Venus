-- Venus hub tables. Not keck `jwst` docs. M3.0: one Y.Doc per room.
-- `workspace_id` / `doc_id` are UUID. The M0 wiki is UUID v5 (DNS) of
-- `venus-m0`; SQL `doc_id` is UUID v5 (DNS) of `doc:home`. Must match
-- `venus_hub::DEFAULT_WORKSPACE_ID` / `PAGE_DOC_ID`.
-- BlockSuite `createDoc` stays `doc:home` (Yjs guid); the hub does not
-- store that string.

CREATE TABLE IF NOT EXISTS crdt_snapshot (
    workspace_id UUID NOT NULL,
    doc_id UUID NOT NULL,
    bin BYTEA NOT NULL,
    clock BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, doc_id)
);

CREATE TABLE IF NOT EXISTS crdt_update (
    workspace_id UUID NOT NULL,
    doc_id UUID NOT NULL,
    seq BIGSERIAL NOT NULL,
    bin BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, doc_id, seq)
);

-- PK (workspace_id, doc_id, seq) already indexes these columns; drop the
-- leftover from earlier schema copies (P14).
DROP INDEX IF EXISTS crdt_update_ws_doc_seq;

CREATE TABLE IF NOT EXISTS blob (
    workspace_id UUID NOT NULL,
    hash TEXT NOT NULL,
    bytes BYTEA NOT NULL,
    PRIMARY KEY (workspace_id, hash)
);

CREATE TABLE IF NOT EXISTS workspace_lease (
    workspace_id UUID PRIMARY KEY,
    owner TEXT NOT NULL,
    lease_until TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS dirty (
    workspace_id UUID NOT NULL,
    doc_id UUID NOT NULL,
    clock BIGINT NOT NULL,
    first_dirty_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, doc_id)
);

-- Fold two UUIDs (256 bits) to one int8 advisory key. All 16+16 bytes
-- participate via XOR. Distinct (ws, doc) can still collide (64-bit
-- pigeonhole). Not hashtext; not the session SCHEMA_MIGRATE_LOCK.
CREATE OR REPLACE FUNCTION venus_doc_lock_key(ws uuid, doc uuid)
RETURNS bigint
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
  SELECT (
    ('x' || encode(substring(uuid_send(ws) from 1 for 8), 'hex'))::bit(64)::bigint
    # ('x' || encode(substring(uuid_send(ws) from 9 for 8), 'hex'))::bit(64)::bigint
    # ('x' || encode(substring(uuid_send(doc) from 1 for 8), 'hex'))::bit(64)::bigint
    # ('x' || encode(substring(uuid_send(doc) from 9 for 8), 'hex'))::bit(64)::bigint
  );
$$;

-- TEXT → UUID for volumes created before this type change. Slug `venus-m0`
-- / `doc:home` map to the v5 constants. Any other non-uuid value fails
-- migrate (wipe `pg-venus-data`).
CREATE OR REPLACE FUNCTION venus_migrate_workspace_uuid(t text) RETURNS uuid
LANGUAGE plpgsql
IMMUTABLE
AS $$
BEGIN
  IF t = 'venus-m0' THEN
    RETURN '77e4a2b1-8b40-5979-a73c-fd4477216d00'::uuid;
  END IF;
  RETURN t::uuid;
EXCEPTION
  WHEN invalid_text_representation THEN
    RAISE EXCEPTION
      'workspace_id % is not a uuid (wipe pg-venus-data or delete the row)', t;
END;
$$;

CREATE OR REPLACE FUNCTION venus_migrate_doc_uuid(t text) RETURNS uuid
LANGUAGE plpgsql
IMMUTABLE
AS $$
BEGIN
  IF t = 'doc:home' THEN
    RETURN '395cd07b-bdb1-5f54-ada8-e9a3fabb6a20'::uuid;
  END IF;
  RETURN t::uuid;
EXCEPTION
  WHEN invalid_text_representation THEN
    RAISE EXCEPTION
      'doc_id % is not a uuid (wipe pg-venus-data or delete the row)', t;
END;
$$;

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = current_schema()
      AND table_name = 'crdt_snapshot'
      AND column_name = 'workspace_id'
      AND data_type = 'text'
  ) THEN
    ALTER TABLE crdt_snapshot
      ALTER COLUMN workspace_id TYPE uuid
        USING venus_migrate_workspace_uuid(workspace_id),
      ALTER COLUMN doc_id TYPE uuid
        USING venus_migrate_doc_uuid(doc_id);
    ALTER TABLE crdt_update
      ALTER COLUMN workspace_id TYPE uuid
        USING venus_migrate_workspace_uuid(workspace_id),
      ALTER COLUMN doc_id TYPE uuid
        USING venus_migrate_doc_uuid(doc_id);
    ALTER TABLE blob
      ALTER COLUMN workspace_id TYPE uuid
        USING venus_migrate_workspace_uuid(workspace_id);
    ALTER TABLE workspace_lease
      ALTER COLUMN workspace_id TYPE uuid
        USING venus_migrate_workspace_uuid(workspace_id);
    ALTER TABLE dirty
      ALTER COLUMN workspace_id TYPE uuid
        USING venus_migrate_workspace_uuid(workspace_id),
      ALTER COLUMN doc_id TYPE uuid
        USING venus_migrate_doc_uuid(doc_id);
  END IF;
END $$;

-- Installed only after `dirty` exists. Inner EXCEPTION so a missing
-- `dirty` table cannot roll back the persist INSERT/UPDATE.
CREATE OR REPLACE FUNCTION venus_mark_dirty() RETURNS trigger AS $$
DECLARE
    clk bigint;
BEGIN
    IF TG_TABLE_NAME = 'crdt_update' THEN
        clk := NEW.seq;
    ELSE
        clk := NEW.clock;
    END IF;
    BEGIN
        INSERT INTO dirty (workspace_id, doc_id, clock, first_dirty_at)
        VALUES (NEW.workspace_id, NEW.doc_id, clk, now())
        ON CONFLICT (workspace_id, doc_id) DO UPDATE
            SET clock = EXCLUDED.clock;
    EXCEPTION
        WHEN undefined_table THEN
            NULL;
        WHEN undefined_column THEN
            NULL;
    END;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- venus:triggers
-- First-install only. `db::migrate` skips this batch when both triggers exist
-- so a second hub boot does not take AccessExclusive on live tables (L20).
-- Lock order matches compact's table grabs (update, then snapshot, then dirty).
LOCK TABLE crdt_update, crdt_snapshot, dirty IN ACCESS EXCLUSIVE MODE;

DROP TRIGGER IF EXISTS crdt_update_dirty ON crdt_update;
CREATE TRIGGER crdt_update_dirty
    AFTER INSERT ON crdt_update
    FOR EACH ROW
    EXECUTE FUNCTION venus_mark_dirty();

DROP TRIGGER IF EXISTS crdt_snapshot_dirty ON crdt_snapshot;
CREATE TRIGGER crdt_snapshot_dirty
    AFTER INSERT OR UPDATE ON crdt_snapshot
    FOR EACH ROW
    EXECUTE FUNCTION venus_mark_dirty();
