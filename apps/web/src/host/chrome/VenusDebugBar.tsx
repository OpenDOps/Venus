import { useEffect, useState } from 'react';
import {
  CATALOG_GIT_LOG_PATH,
  nextGitLogPoll,
  parseGitLog,
  sameGitLogShas,
} from './git-log.js';

type Props = {
  sidecarUrl: string;
};

function statusSha(data: unknown): string | null {
  if (!data || typeof data !== 'object') return null;
  const sha = (data as { sha?: unknown }).sha;
  return typeof sha === 'string' && sha.length > 0 ? sha : null;
}

function flushErrorFromStatus(data: unknown): string | null {
  if (!data || typeof data !== 'object') return null;
  const row = data as { last_error?: unknown; failed?: unknown };
  if (typeof row.last_error !== 'string' || !row.last_error.trim()) return null;
  return row.last_error.trim();
}

export function VenusDebugBar({ sidecarUrl }: Props) {
  const [gitLog, setGitLog] = useState(() => parseGitLog([]));
  const [flushError, setFlushError] = useState<string | null>(null);

  useEffect(() => {
    const loadGitLog = () => {
      void fetch(`${sidecarUrl}/git/log?path=${CATALOG_GIT_LOG_PATH}`)
        .then((res) => (res.ok ? res.json() : []))
        .then((data) => {
          const next = parseGitLog(data);
          setGitLog((prev) => (sameGitLogShas(prev, next) ? prev : next));
        })
        .catch((err) => {
          console.error(err);
        });
    };
    let poll: { seen: boolean; sha: string | null } = { seen: false, sha: null };
    const loadFlushStatus = () => {
      void fetch(`${sidecarUrl}/flush/status`)
        .then((res) => (res.ok ? res.json() : null))
        .then((data) => {
          setFlushError(flushErrorFromStatus(data));
          const sha = statusSha(data);
          const next = nextGitLogPoll(poll, sha);
          poll = { seen: next.seen, sha: next.sha };
          if (next.refresh) loadGitLog();
        })
        .catch((err) => {
          console.error(err);
        });
    };
    loadGitLog();
    loadFlushStatus();
    const id = window.setInterval(loadFlushStatus, 2000);
    return () => window.clearInterval(id);
  }, [sidecarUrl]);

  function onFlush() {
    void fetch(`${sidecarUrl}/flush`, { method: 'POST' })
      .then((res) => (res.status === 409 ? res.text() : null))
      .then((text) => {
        const message = text?.trim();
        if (message) setFlushError(message);
      })
      .catch((err) => {
        console.error(err);
      });
  }

  return (
    <div className="host-chrome">
      <button type="button" data-testid="venus-flush" onClick={onFlush}>
        Flush
      </button>
      {flushError ? (
        <span
          className="host-chrome-error"
          data-testid="venus-flush-error"
          title={flushError}
        >
          {flushError}
        </span>
      ) : null}
      <ol className="host-chrome-log" data-testid="venus-git-log">
        {gitLog.map((row) => (
          <li key={row.sha} data-sha={row.sha}>
            {row.subject}
          </li>
        ))}
      </ol>
    </div>
  );
}
