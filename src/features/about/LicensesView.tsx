import { useEffect, useId, useMemo, useState } from "react";
import { Copy, ExternalLink, Search } from "lucide-react";
import {
  CATEGORY_LABEL,
  componentKey,
  filterComponents,
  licenseLabel,
  licenseOptions,
  loadLicenseIndex,
  loadLicenseTexts,
  type ComponentCategory,
  type LicenseIndex,
  type LicensedComponent,
} from "./licenses";

interface Props {
  openProjectUrl: (url: string) => Promise<unknown>;
  copyText: (text: string) => Promise<void>;
  notify: (tone: "success" | "error", message: string) => void;
  /** For tests: replaces the bundled data. */
  loadIndex?: () => Promise<LicenseIndex>;
  loadTexts?: () => Promise<string[]>;
}

export function LicensesView({ openProjectUrl, copyText, notify, loadIndex = loadLicenseIndex, loadTexts = loadLicenseTexts }: Props) {
  const [index, setIndex] = useState<LicenseIndex | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState("");
  const [license, setLicense] = useState<string | null>(null);
  const [category, setCategory] = useState<ComponentCategory | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const ids = { search: useId(), license: useId(), category: useId(), list: useId() };

  useEffect(() => {
    loadIndex()
      .then(setIndex)
      .catch(() => setFailed(true));
  }, [loadIndex]);

  const components = index?.components ?? [];
  const visible = useMemo(() => filterComponents(components, { query, license, category }), [components, query, license, category]);
  const options = useMemo(() => licenseOptions(components), [components]);
  const current = visible.find((c) => componentKey(c) === selected) ?? null;

  if (failed) return <p className="nx-notice is-danger">The license information could not be loaded. The same information is in THIRD-PARTY-NOTICES.txt next to Nexdir.exe.</p>;
  if (!index) return <p className="nx-muted">Loading license information…</p>;

  const counts = { native: 0, rust: 0, javascript: 0 };
  for (const c of components) counts[c.category]++;

  return (
    <div className="nx-licenses">
      <p className="nx-muted">
        Nexdir includes {components.length} open-source components ({counts.rust} Rust crates, {counts.javascript} JavaScript packages, {counts.native} native libraries). Their licenses are
        listed here and in THIRD-PARTY-NOTICES.txt, available offline.
      </p>
      <div className="nx-licenses-filters">
        <div className="nx-licenses-search">
          <Search size={15} aria-hidden="true" />
          <label htmlFor={ids.search} className="nx-visually-hidden">
            Search components
          </label>
          <input id={ids.search} className="nx-input" type="search" placeholder="Search by name, license or author" value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <label htmlFor={ids.license} className="nx-visually-hidden">
          Filter by license
        </label>
        <select id={ids.license} className="nx-input" value={license ?? ""} onChange={(e) => setLicense(e.target.value || null)}>
          <option value="">All licenses</option>
          {options.map((o) => (
            <option key={o.license} value={o.license}>
              {licenseLabel(o.license)} ({o.count})
            </option>
          ))}
        </select>
        <label htmlFor={ids.category} className="nx-visually-hidden">
          Filter by type
        </label>
        <select id={ids.category} className="nx-input" value={category ?? ""} onChange={(e) => setCategory((e.target.value || null) as ComponentCategory | null)}>
          <option value="">All types</option>
          <option value="rust">Rust crates</option>
          <option value="javascript">JavaScript packages</option>
          <option value="native">Native libraries</option>
        </select>
      </div>

      <div className="nx-licenses-body">
        <div className="nx-licenses-list" role="listbox" id={ids.list} aria-label={`Components, ${visible.length} shown`} tabIndex={visible.length ? -1 : undefined}>
          {visible.length === 0 && <p className="nx-muted nx-licenses-empty">No component matches.</p>}
          {visible.map((c) => {
            const key = componentKey(c);
            return (
              <button
                key={key}
                type="button"
                role="option"
                aria-selected={key === selected}
                className={`nx-licenses-item${key === selected ? " is-selected" : ""}`}
                onClick={() => setSelected(key)}
                onKeyDown={(e) => {
                  const i = visible.indexOf(c);
                  const next = e.key === "ArrowDown" ? visible[i + 1] : e.key === "ArrowUp" ? visible[i - 1] : undefined;
                  if (next) {
                    e.preventDefault();
                    setSelected(componentKey(next));
                    (e.currentTarget.parentElement?.children[i + (e.key === "ArrowDown" ? 1 : -1)] as HTMLElement | undefined)?.focus();
                  }
                }}
              >
                <span className="nx-licenses-name">{c.name}</span>
                <span className="nx-licenses-meta">
                  {c.version} · {licenseLabel(c.license)}
                </span>
              </button>
            );
          })}
        </div>
        <div className="nx-licenses-detail" aria-live="polite">
          {current ? (
            <LicenseDetail key={componentKey(current)} component={current} loadTexts={loadTexts} openProjectUrl={openProjectUrl} copyText={copyText} notify={notify} />
          ) : (
            <p className="nx-muted">Select a component to see its license.</p>
          )}
        </div>
      </div>

      {index.devTools.length > 0 && (
        <details className="nx-help-topic">
          <summary>Development tools (not distributed with Nexdir)</summary>
          <div className="nx-help-topic-body">
            <ul className="nx-credits-list">
              {index.devTools.map((t) => (
                <li key={t.name}>
                  <strong>{t.name}</strong> {t.version} · {t.license}
                </li>
              ))}
            </ul>
          </div>
        </details>
      )}
    </div>
  );
}

function LicenseDetail({
  component,
  loadTexts,
  openProjectUrl,
  copyText,
  notify,
}: {
  component: LicensedComponent;
  loadTexts: () => Promise<string[]>;
  openProjectUrl: (url: string) => Promise<unknown>;
  copyText: (text: string) => Promise<void>;
  notify: (tone: "success" | "error", message: string) => void;
}) {
  const [texts, setTexts] = useState<string[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadTexts()
      .then((all) => !cancelled && setTexts(component.texts.map((i) => all[i] ?? "")))
      .catch(() => !cancelled && setFailed(true));
    return () => {
      cancelled = true;
    };
  }, [component, loadTexts]);

  const full = texts?.join("\n\n" + "-".repeat(60) + "\n\n") ?? "";

  return (
    <article className="nx-license-detail">
      <h3>
        {component.name} <span className="nx-muted">{component.version}</span>
      </h3>
      <dl className="nx-update-facts is-compact">
        <div>
          <dt>License</dt>
          <dd>{licenseLabel(component.license)}</dd>
        </div>
        <div>
          <dt>Type</dt>
          <dd>{CATEGORY_LABEL[component.category]}</dd>
        </div>
        {component.authors.length > 0 && (
          <div>
            <dt>Developed by</dt>
            <dd>{component.authors.join(", ")}</dd>
          </div>
        )}
        <div>
          <dt>Project</dt>
          <dd className="nx-license-source">{component.source}</dd>
        </div>
      </dl>
      {component.note && <p className="nx-muted">{component.note}</p>}
      {component.copyleft && <p className="nx-notice is-info">{component.copyleft}</p>}
      <div className="nx-inline-actions">
        <button type="button" className="nx-button is-small" onClick={() => void openProjectUrl(component.source).catch(() => notify("error", "Could not open the project page."))}>
          <ExternalLink size={14} /> Open Project Page
        </button>
        <button
          type="button"
          className="nx-button is-small"
          disabled={!texts}
          onClick={() =>
            copyText(`${component.name} ${component.version} — ${component.license}\n\n${full}`)
              .then(() => notify("success", "License text copied."))
              .catch(() => notify("error", "Could not access the clipboard."))
          }
        >
          <Copy size={14} /> Copy License Text
        </button>
      </div>
      {failed ? (
        <p className="nx-notice is-danger">The license text could not be loaded. It is also in THIRD-PARTY-NOTICES.txt.</p>
      ) : texts ? (
        <pre className="nx-license-text" tabIndex={0} aria-label={`License text of ${component.name}`}>
          {full}
        </pre>
      ) : (
        <p className="nx-muted">Loading license text…</p>
      )}
    </article>
  );
}
