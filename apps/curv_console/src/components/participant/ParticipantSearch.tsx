"use client";

import Link from "next/link";
import { useRef, useState, type FormEvent } from "react";
import type { ParticipantSearchPage, ParticipantSearchRow } from "@/lib/api/emr";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { AGE_GROUPS } from "@/messages/participant";
import { ageBracketLabel } from "./labels";
import { failureText, type ActionResult } from "./types";
import { orgPaths } from "@/lib/paths";

type Search = (form: FormData) => Promise<ActionResult<{ page: ParticipantSearchPage }>>;

/**
 * Participant search (REC-2, UI-4). A POST form handled by a server action: the filters (the MRN too, for roles that
 * may use it) are never in the address bar, the history or a log line, and the results come back as data. "Show more"
 * re-sends the same form with the cursor. The MRN field is cleared once the search is sent.
 */
export function ParticipantSearch({ search, canMrn, orgPath }: { search: Search; canMrn: boolean; orgPath: string }) {
  const text = useText();
  const t = text.participant.search;
  const c = t.cols;
  const formRef = useRef<HTMLFormElement>(null);
  const last = useRef<FormData | null>(null);
  const [pending, setPending] = useState(false);
  const [rows, setRows] = useState<ParticipantSearchRow[] | null>(null);
  const [total, setTotal] = useState<number | null | undefined>(null);
  const [next, setNext] = useState<string | null>(null);
  const [failure, setFailure] = useState<ActionResult | null>(null);

  async function run(data: FormData, append: boolean) {
    setPending(true);
    setFailure(null);
    let r: ActionResult<{ page: ParticipantSearchPage }>;
    try {
      r = await search(data);
    } catch {
      r = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (!r.ok) {
      setFailure(r);
      return;
    }
    setRows((prev) => (append && prev ? [...prev, ...r.page.items] : r.page.items));
    setTotal(r.page.total);
    setNext(r.page.next_cursor ?? null);
  }

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    last.current = data;
    const mrn = formRef.current?.elements.namedItem("mrn");
    if (mrn instanceof HTMLInputElement) mrn.value = "";
    void run(data, false);
  }

  function more() {
    if (!last.current || !next) return;
    const data = new FormData();
    for (const [k, v] of last.current.entries()) if (k !== "cursor") data.append(k, v);
    data.set("cursor", next);
    void run(data, true);
  }

  function clear() {
    formRef.current?.reset();
    last.current = null;
    setRows(null);
    setNext(null);
    setFailure(null);
  }

  return (
    <section className="panel stack" aria-labelledby="search-h">
      <h2 id="search-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      <form ref={formRef} method="post" onSubmit={onSubmit} className="timeline-filter" aria-label={t.title} autoComplete="off">
        <label>
          {t.token}
          <input name="token_prefix" maxLength={40} placeholder={t.tokenHelp} />
        </label>
        <label>
          {t.age}
          <select name="age_bracket" defaultValue="">
            <option value="">{t.any}</option>
            {AGE_GROUPS.map((a) => (
              <option key={a} value={a}>{ageBracketLabel(text.locale, a)}</option>
            ))}
          </select>
        </label>
        <label>
          {t.lastSeenFrom}
          <input type="date" name="last_seen_from" />
        </label>
        <label>
          {t.lastSeenTo}
          <input type="date" name="last_seen_to" />
        </label>
        {canMrn ? (
          <label>
            {t.mrn}
            <input name="mrn" maxLength={64} />
          </label>
        ) : null}
        <label className="check">
          <input type="checkbox" name="has_open_follow_up" value="true" /> {t.openFollowUp}
        </label>
        <div className="filter-actions">
          <button type="submit" disabled={pending}>{pending ? t.searching : t.submit}</button>
          <button type="button" className="secondary" onClick={clear} disabled={pending}>{t.clear}</button>
        </div>
      </form>
      {failure && !failure.ok ? <p role="alert" className="error">{t.failed} {failureText(failure, text.locale)}</p> : null}
      {rows === null ? null : rows.length === 0 ? (
        <p role="status" className="muted">{t.none}</p>
      ) : (
        <>
          <p role="status" className="muted">{t.results(rows.length, total)}</p>
          <div className="table-wrap">
            <table className="data responsive" aria-label={t.title}>
              <thead>
                <tr>
                  <th scope="col">{c.token}</th>
                  <th scope="col">{c.age}</th>
                  <th scope="col">{c.sessions}</th>
                  <th scope="col">{c.lastSession}</th>
                  <th scope="col">{c.openVisits}</th>
                  <th scope="col">{c.nextFollowUp}</th>
                  <th scope="col">{c.status}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((p) => (
                  <tr key={p.id} data-testid="search-row">
                    <td data-label={c.token}>
                      <Link href={orgPaths(orgPath).clinic.participant(p.id)}>{p.participant_token}</Link>
                    </td>
                    <td data-label={c.age}>{p.age_bracket_at_enrol ? ageBracketLabel(text.locale, p.age_bracket_at_enrol) : "-"}</td>
                    <td data-label={c.sessions}>{p.session_count}</td>
                    <td data-label={c.lastSession}>{p.last_session_at ? formatDateTime(p.last_session_at) : "-"}</td>
                    <td data-label={c.openVisits}>{p.open_encounters_n ?? 0}</td>
                    <td data-label={c.nextFollowUp}>{p.next_follow_up_at ? formatDateTime(p.next_follow_up_at) : "-"}</td>
                    <td data-label={c.status}>
                      <span className={`badge ${p.withdrawn_at ? "badge-unreliable" : "badge-green"}`}>
                        {p.withdrawn_at ? text.participant.header.withdrawn : text.participant.header.active}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {next ? (
            <button type="button" className="secondary" onClick={more} disabled={pending}>{t.more}</button>
          ) : null}
        </>
      )}
    </section>
  );
}
