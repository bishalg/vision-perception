import { en } from "@/messages/en";

/** Accessible horizontal bar list for a label -> count map. */
export function Distribution({ title, counts }: { title: string; counts: Record<string, number> }) {
  const entries = Object.entries(counts).sort((a, b) => b[1] - a[1]);
  const max = Math.max(1, ...entries.map(([, n]) => n));
  return (
    <section className="panel" aria-label={title}>
      <h2>{title}</h2>
      {entries.length === 0 ? (
        <p className="muted">{en.overview.none}</p>
      ) : (
        <ul className="bars">
          {entries.map(([label, n]) => (
            <li key={label}>
              <span className="bar-label">{label}</span>
              <span className="bar-track" aria-hidden="true">
                <span className="bar-fill" style={{ width: `${(n / max) * 100}%` }} />
              </span>
              <span className="bar-count">{n}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
