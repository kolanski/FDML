import { useState, useEffect, useRef } from 'react';

interface GenerationStatus {
  phase: string;
  progress: number;
  message: string;
}

interface GenerationViewProps {
  onComplete: () => void;
}

export default function GenerationView({ onComplete }: GenerationViewProps) {
  const [logs, setLogs] = useState<string[]>([]);
  const [status, setStatus] = useState<GenerationStatus>({ phase: 'scanning', progress: 0, message: 'Starting...' });
  const logEndRef = useRef<HTMLDivElement>(null);
  const hasCompleted = useRef(false);

  // Poll generation status
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const res = await fetch('/api/generation-status');
        if (res.ok) {
          const s: GenerationStatus = await res.json();
          setStatus(s);
          if ((s.phase === 'done' || s.phase === 'idle') && !hasCompleted.current) {
            hasCompleted.current = true;
            setTimeout(() => onComplete(), 1500);
          }
        }
      } catch { /* ignore */ }
    }, 1000);
    return () => clearInterval(interval);
  }, [onComplete]);

  // SSE log stream
  useEffect(() => {
    const es = new EventSource('/api/generation-logs');
    es.addEventListener('log', (e) => {
      setLogs((prev) => [...prev, e.data]);
    });
    es.onerror = () => {
      setTimeout(() => es.close(), 5000);
    };
    return () => es.close();
  }, []);

  // Auto-scroll
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const phaseColor =
    status.phase === 'done' ? '#16a34a'
    : status.phase === 'error' ? '#dc2626'
    : '#3b82f6';

  const progressPct = Math.round(status.progress * 100);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0d1117', color: '#c9d1d9' }}>
      {/* Header */}
      <div style={{
        padding: '16px 24px',
        borderBottom: '1px solid #21262d',
        display: 'flex',
        alignItems: 'center',
        gap: 16,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {status.phase !== 'done' && status.phase !== 'error' && (
            <div style={{
              width: 12, height: 12, borderRadius: '50%',
              border: '2px solid #3b82f6',
              borderTopColor: 'transparent',
              animation: 'spin 1s linear infinite',
            }} />
          )}
          <span style={{ fontWeight: 700, fontSize: 14, color: phaseColor, textTransform: 'uppercase' }}>
            {status.phase}
          </span>
        </div>
        <span style={{ fontSize: 13, color: '#8b949e' }}>{status.message}</span>
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 12, color: '#8b949e', fontFamily: 'monospace' }}>{progressPct}%</span>
      </div>

      {/* Progress bar */}
      <div style={{ height: 3, background: '#21262d' }}>
        <div style={{
          height: '100%',
          width: `${progressPct}%`,
          background: phaseColor,
          transition: 'width 0.5s ease',
        }} />
      </div>

      {/* Log output */}
      <div style={{
        flex: 1, overflow: 'auto', padding: '12px 24px',
        fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
        fontSize: 12, lineHeight: 1.7,
      }}>
        {logs.map((line, i) => (
          <div key={i} style={{
            color: line.startsWith('ERROR') ? '#f85149'
              : line.startsWith('Warning') ? '#d29922'
              : line.includes('Got ') || line.includes('complete') || line.includes('Written') ? '#3fb950'
              : '#c9d1d9',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
          }}>
            <span style={{ color: '#484f58', marginRight: 8, userSelect: 'none' }}>
              {String(i + 1).padStart(3)}
            </span>
            {line}
          </div>
        ))}
        <div ref={logEndRef} />
      </div>

      <style>{`
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
