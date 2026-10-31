import { useCallback, useEffect, useRef, useState } from "react";
import { normalizeError } from "../api/client";
import type { UnityError } from "../api/types";

interface AsyncState<T> {
  data: T | null;
  error: UnityError | null;
  loading: boolean;
}

export function useAsync<T>(loader: () => Promise<T>, deps: unknown[] = []) {
  const loaderRef = useRef(loader);
  loaderRef.current = loader;
  const [state, setState] = useState<AsyncState<T>>({
    data: null,
    error: null,
    loading: true,
  });
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    let alive = true;
    setState((previous) => ({ ...previous, loading: true, error: null }));
    loaderRef
      .current()
      .then((data) => {
        if (alive) {
          setState({ data, error: null, loading: false });
        }
      })
      .catch((error: unknown) => {
        if (alive) {
          setState((previous) => ({
            data: previous.data,
            error: normalizeError(error),
            loading: false,
          }));
        }
      });
    return () => {
      alive = false;
    };
    // `deps` is intentionally part of the effect contract.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, nonce]);

  const reload = useCallback(() => setNonce((value) => value + 1), []);
  const setData = useCallback(
    (data: T | null) => setState((previous) => ({ ...previous, data })),
    [],
  );

  return { ...state, reload, setData };
}
