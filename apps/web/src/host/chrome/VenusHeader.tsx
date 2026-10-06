import { useEffect, useRef, useState } from 'react';
import { testidProps } from '../providers/from-env.js';
import type { SyncProvider } from '../sync-provider.js';
import { createTitleCommit } from './title-commit.js';
import './VenusHeader.css';

type HistorySignal = {
  peek: () => boolean;
  subscribe: (fn: (value: boolean) => void) => () => void;
};

export type HeaderStore = {
  undo: () => void;
  redo: () => void;
  history: {
    canUndo$: HistorySignal;
    canRedo$: HistorySignal;
  };
};

type Props = {
  store: HeaderStore;
  title: string;
  onTitleChange: (title: string) => string | void;
  provider: SyncProvider;
};

function connectionOf(provider: SyncProvider) {
  return provider.connection === 'reconnecting' ? 'reconnecting' : 'synced';
}

export function VenusHeader({ store, title, onTitleChange, provider }: Props) {
  const [canUndo, setCanUndo] = useState(() =>
    store.history.canUndo$.peek(),
  );
  const [canRedo, setCanRedo] = useState(() =>
    store.history.canRedo$.peek(),
  );
  const [draft, setDraft] = useState(title);
  const [connection, setConnection] = useState(() => connectionOf(provider));
  const focusedRef = useRef(false);
  const onTitleChangeRef = useRef(onTitleChange);
  onTitleChangeRef.current = onTitleChange;
  const commitRef = useRef<ReturnType<typeof createTitleCommit> | null>(null);
  if (commitRef.current == null) {
    commitRef.current = createTitleCommit((next) => {
      const shown = onTitleChangeRef.current(next);
      if (typeof shown === 'string') setDraft(shown);
    });
  }

  useEffect(() => {
    setConnection(connectionOf(provider));
    return provider.on?.('connection', () => {
      setConnection(connectionOf(provider));
    });
  }, [provider]);

  useEffect(() => {
    if (connection !== 'reconnecting') return;
    const onLeave = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', onLeave);
    return () => window.removeEventListener('beforeunload', onLeave);
  }, [connection]);

  useEffect(() => {
    const undo$ = store.history.canUndo$;
    const redo$ = store.history.canRedo$;
    setCanUndo(undo$.peek());
    setCanRedo(redo$.peek());
    const offUndo = undo$.subscribe(setCanUndo);
    const offRedo = redo$.subscribe(setCanRedo);
    return () => {
      offUndo();
      offRedo();
    };
  }, [store]);

  useEffect(() => {
    return () => commitRef.current?.cancel();
  }, []);

  useEffect(() => {
    if (focusedRef.current) return;
    commitRef.current?.cancel();
    setDraft(title);
  }, [title]);

  return (
    <header className="venus-header" {...testidProps('venus-header')}>
      <button
        type="button"
        disabled={!canUndo}
        onClick={() => store.undo()}
        {...testidProps('venus-undo')}
      >
        Undo
      </button>
      <button
        type="button"
        disabled={!canRedo}
        onClick={() => store.redo()}
        {...testidProps('venus-redo')}
      >
        Redo
      </button>
      {connection === 'reconnecting' ? (
        <span
          className="venus-connection"
          title="Offline. Reconnecting to the hub."
          {...testidProps('venus-connection')}
        >
          reconnecting
        </span>
      ) : null}
      <input
        type="text"
        className="venus-page-title"
        value={draft}
        aria-label="Page title"
        autoComplete="off"
        onFocus={() => {
          focusedRef.current = true;
        }}
        onChange={(event) => {
          const next = event.target.value;
          setDraft(next);
          commitRef.current?.push(next);
        }}
        onBlur={() => {
          focusedRef.current = false;
          commitRef.current?.flush();
        }}
        onKeyDown={(event) => {
          if (event.key !== 'Enter') return;
          event.preventDefault();
          commitRef.current?.flush();
          event.currentTarget.blur();
        }}
        {...testidProps('venus-page-title')}
      />
    </header>
  );
}
