import { useEffect, useState } from 'react';
import {
  CATALOG_GIT_LOG_PATH,
  parseGitLog,
  sameGitLogShas,
} from './git-log.js';

type Props = {
  sidecarUrl: string;
};

export function VenusDebugBar({ sidecarUrl }: Props) {
  const [gitLog, setGitLog] = useState(() => parseGitLog([]));

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
    loadGitLog();
    const id = window.setInterval(loadGitLog, 2000);
    return () => window.clearInterval(id);
  }, [sidecarUrl]);

  function onFlush() {
    void fetch(`${sidecarUrl}/flush`, { method: 'POST' }).catch((err) => {
      console.error(err);
    });
  }

  return (
    <div className="host-chrome">
      <button type="button" data-testid="venus-flush" onClick={onFlush}>
        Flush
      </button>
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
