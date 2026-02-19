import { useState, useEffect, useCallback } from 'react';
import type { FdmlDocument } from '../api/types';
import { getSpec, subscribeEvents } from '../api/client';

export function useSpec() {
  const [spec, setSpec] = useState<FdmlDocument | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchSpec = useCallback(async () => {
    try {
      const doc = await getSpec();
      setSpec(doc);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSpec();
    const unsub = subscribeEvents(() => fetchSpec());
    return unsub;
  }, [fetchSpec]);

  return { spec, error, loading };
}
