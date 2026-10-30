export function KpiCard({ label, value, hint }: { label: string; value: number | string; hint?: string }) {
  return (
    <section className="kpi" aria-label={label}>
      <div className="kpi-label">{label}</div>
      <div className="kpi-value">{value}</div>
      {hint ? <div className="kpi-hint">{hint}</div> : null}
    </section>
  );
}
