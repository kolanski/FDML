import type { FdmlDocument } from './types';

export async function getSpec(): Promise<FdmlDocument> {
  const res = await fetch('/api/spec');
  if (!res.ok) throw new Error(`Failed to fetch spec: ${res.status}`);
  return res.json();
}

export function subscribeEvents(onChanged: () => void): () => void {
  const es = new EventSource('/api/events');
  es.addEventListener('spec-changed', () => onChanged());
  es.onerror = () => {
    // Reconnect after brief delay
    setTimeout(() => {
      es.close();
      subscribeEvents(onChanged);
    }, 2000);
  };
  return () => es.close();
}
