import { useEffect, useRef, useState } from 'react';
import { testidProps } from '../providers/from-env.js';
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
  onTitleChange: (title: string) => void;
};

export function VenusHeader({ store, title, onTitleChange }: Props) {
  const [canUndo, setCanUndo] = useState(() =>
    store.history.canUndo$.peek(),
  );
  const [canRedo, setCanRedo] = useState(() =>
    store.history.canRedo$.peek(),
  );
  const [draft, setDraft] = useState(title);
  const focusedRef = useRef(false);

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
    if (!focusedRef.current) setDraft(title);
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
          onTitleChange(next);
        }}
        onBlur={() => {
          focusedRef.current = false;
          setDraft(title);
        }}
        onKeyDown={(event) => {
          if (event.key !== 'Enter') return;
          event.preventDefault();
          event.currentTarget.blur();
        }}
        {...testidProps('venus-page-title')}
      />
    </header>
  );
}
