#!/bin/bash
# Create NOSUPERUSER role venus_hub and hand it the hub tables.
# Official postgres images make POSTGRES_USER a superuser; the hub must not
# use that role. Runs from docker-entrypoint-initdb.d (new volumes) and from
# the postgres healthcheck (existing volumes — init scripts do not re-run).
set -euo pipefail

DB="${POSTGRES_DB:-venus}"
ADMIN="${POSTGRES_USER:-venus}"
PASS="${POSTGRES_PASSWORD:-venus}"
APP_ROLE="venus_hub"

ident_ok() {
  case "$1" in
    '' | *[!a-zA-Z0-9_]* | [0-9]*) return 1 ;;
    *) return 0 ;;
  esac
}
ident_ok "$DB" || { echo "ensure-app-role: bad POSTGRES_DB" >&2; exit 1; }
ident_ok "$ADMIN" || { echo "ensure-app-role: bad POSTGRES_USER" >&2; exit 1; }
ident_ok "$APP_ROLE" || { echo "ensure-app-role: bad APP_ROLE" >&2; exit 1; }

sql_quote() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/''/g")"
}

PW_SQL="$(sql_quote "$PASS")"

psql -v ON_ERROR_STOP=1 --username "$ADMIN" --dbname "$DB" <<SQL
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${APP_ROLE}') THEN
    CREATE ROLE ${APP_ROLE} LOGIN PASSWORD ${PW_SQL}
      NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
  ELSE
    ALTER ROLE ${APP_ROLE} LOGIN PASSWORD ${PW_SQL}
      NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
  END IF;
END
\$\$;

GRANT CONNECT ON DATABASE ${DB} TO ${APP_ROLE};
GRANT USAGE, CREATE ON SCHEMA public TO ${APP_ROLE};
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO ${APP_ROLE};
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO ${APP_ROLE};
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO ${APP_ROLE};
ALTER DEFAULT PRIVILEGES FOR ROLE ${ADMIN} IN SCHEMA public
  GRANT ALL ON TABLES TO ${APP_ROLE};
ALTER DEFAULT PRIVILEGES FOR ROLE ${ADMIN} IN SCHEMA public
  GRANT ALL ON SEQUENCES TO ${APP_ROLE};
ALTER DEFAULT PRIVILEGES FOR ROLE ${APP_ROLE} IN SCHEMA public
  GRANT ALL ON TABLES TO ${APP_ROLE};
ALTER DEFAULT PRIVILEGES FOR ROLE ${APP_ROLE} IN SCHEMA public
  GRANT ALL ON SEQUENCES TO ${APP_ROLE};

ALTER TABLE IF EXISTS crdt_snapshot OWNER TO ${APP_ROLE};
ALTER TABLE IF EXISTS crdt_update OWNER TO ${APP_ROLE};
ALTER TABLE IF EXISTS blob OWNER TO ${APP_ROLE};
ALTER TABLE IF EXISTS workspace_lease OWNER TO ${APP_ROLE};
ALTER TABLE IF EXISTS dirty OWNER TO ${APP_ROLE};
ALTER SEQUENCE IF EXISTS crdt_update_seq OWNER TO ${APP_ROLE};
SQL

psql -v ON_ERROR_STOP=1 --username "$ADMIN" --dbname "$DB" <<'SQL'
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'venus_mark_dirty'
      AND pg_function_is_visible(p.oid)
  ) THEN
    ALTER FUNCTION venus_mark_dirty() OWNER TO venus_hub;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'venus_doc_lock_key'
  ) THEN
    ALTER FUNCTION venus_doc_lock_key(uuid, uuid) OWNER TO venus_hub;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'venus_migrate_workspace_uuid'
  ) THEN
    ALTER FUNCTION venus_migrate_workspace_uuid(text) OWNER TO venus_hub;
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'venus_migrate_doc_uuid'
  ) THEN
    ALTER FUNCTION venus_migrate_doc_uuid(text) OWNER TO venus_hub;
  END IF;
END
$$;
SQL
