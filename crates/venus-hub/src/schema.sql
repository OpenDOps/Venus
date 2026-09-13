-- Venus hub tables. Not keck `jwst` docs. M3.0: one Y.Doc per room.
-- `workspace_id` / `doc_id` are UUID. The M0 wiki is UUID v5 (DNS) of
-- `venus-m0`; SQL `doc_id` is UUID v5 (DNS) of `doc:home`. Must match
-- `venus_hub::DEFAULT_WORKSPACE_ID` / `PAGE_DOC_ID`.
-- BlockSuite `createDoc` stays `doc:home` (Yjs guid); the hub does not
-- store that string.
-- Dirty grain is (workspace_id, doc_id): one row per page. M3 many pages
-- keep that pair; lease grain stays workspace_id. The hub does not
-- write `jobs` — M3 observer reads `dirty`.

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

-- Page grain. Hub never DELETEs these rows; M3 observer GC may.
-- Only `crdt_update` marks dirty, so compact cannot resurrect a GCed row.
-- M3 still ignores `clock <= last_flushed` (a retried flush is not a new pin).
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
-- `dirty` table cannot roll back persist (one savepoint per statement,
-- not per row — P13). ROW branch is only for the boot that recreates
-- leftover FOR EACH ROW triggers as STATEMENT.
-- P1: only `crdt_update` marks dirty. Compact rewrites rows that already
-- marked it, so the whole-page snapshot `bin` never enters a NEW TABLE.
-- A leftover crdt_snapshot trigger is a no-op until `migrate` drops it.
-- D1: GREATEST so an out-of-order flush statement cannot rewind the clock.
-- D2: RAISE WARNING (do not RAISE EXCEPTION).
-- D4: pin search_path; qualify public.dirty (do not follow "$user").
CREATE OR REPLACE FUNCTION venus_mark_dirty() RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
BEGIN
    IF TG_TABLE_NAME <> 'crdt_update' THEN
        IF TG_LEVEL = 'ROW' THEN
            RETURN NEW;
        END IF;
        RETURN NULL;
    END IF;
    BEGIN
        IF TG_LEVEL = 'STATEMENT' THEN
            INSERT INTO public.dirty (workspace_id, doc_id, clock, first_dirty_at)
            SELECT workspace_id, doc_id, max(seq), now()
            FROM ins
            GROUP BY workspace_id, doc_id
            ON CONFLICT (workspace_id, doc_id) DO UPDATE
                SET clock = GREATEST(dirty.clock, EXCLUDED.clock)
                WHERE dirty.clock IS DISTINCT FROM GREATEST(dirty.clock, EXCLUDED.clock);
        ELSE
            INSERT INTO public.dirty (workspace_id, doc_id, clock, first_dirty_at)
            VALUES (NEW.workspace_id, NEW.doc_id, NEW.seq, now())
            ON CONFLICT (workspace_id, doc_id) DO UPDATE
                SET clock = GREATEST(dirty.clock, EXCLUDED.clock)
                WHERE dirty.clock IS DISTINCT FROM GREATEST(dirty.clock, EXCLUDED.clock);
        END IF;
    EXCEPTION
        WHEN undefined_table OR undefined_column THEN
            IF TG_LEVEL = 'ROW' THEN
                RAISE WARNING 'venus_mark_dirty: % (workspace_id=%, doc_id=%)',
                    SQLERRM, NEW.workspace_id, NEW.doc_id;
            ELSE
                RAISE WARNING 'venus_mark_dirty: % (table=%)', SQLERRM, TG_TABLE_NAME;
            END IF;
    END;
    IF TG_LEVEL = 'ROW' THEN
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$;

-- venus:triggers
-- First-install, cutover from FOR EACH ROW (P13: tgnewtable set), or removal
-- of the snapshot triggers (P1). `db::migrate` skips this batch once
-- `crdt_update_dirty` is STATEMENT and no snapshot dirty trigger is left, so a
-- second hub boot does not take AccessExclusive on live tables (L20).
-- Lock order matches compact's table grabs (update, then snapshot, then dirty).
LOCK TABLE crdt_update, crdt_snapshot, dirty IN ACCESS EXCLUSIVE MODE;

DROP TRIGGER IF EXISTS crdt_update_dirty ON crdt_update;
CREATE TRIGGER crdt_update_dirty
    AFTER INSERT ON crdt_update
    REFERENCING NEW TABLE AS ins
    FOR EACH STATEMENT
    EXECUTE FUNCTION venus_mark_dirty();

-- P1: no snapshot trigger. Compact merges trail rows that `crdt_update_dirty`
-- already marked at the same clock (`crdt_snapshot.clock` = that `max_seq`), so
-- these only copied the whole page `bin` into a transition table to rewrite one
-- bigint. `crdt_snapshot_dirty` is the pre-P13 name.
DROP TRIGGER IF EXISTS crdt_snapshot_dirty ON crdt_snapshot;
DROP TRIGGER IF EXISTS crdt_snapshot_dirty_ins ON crdt_snapshot;
DROP TRIGGER IF EXISTS crdt_snapshot_dirty_upd ON crdt_snapshot;
