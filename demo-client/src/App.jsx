import {
  Activity,
  ArrowRight,
  Database,
  HelpCircle,
  Play,
  RotateCcw,
  Sparkles,
  Table2,
  TerminalSquare,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

const API_BASE = "";

const EXAMPLES = [
  {
    title: "Create table",
    sql: "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT);",
    hint: "Defines a table and persists catalog metadata.",
  },
  {
    title: "Insert rows",
    sql: "INSERT INTO users VALUES (1, 'Ada', 28);",
    hint: "Writes a row through SQL executor into the storage engine.",
  },
  {
    title: "Query projection",
    sql: "SELECT name, age FROM users WHERE id = 1;",
    hint: "Shows row projection and predicate evaluation.",
  },
  {
    title: "Update data",
    sql: "UPDATE users SET age = 29 WHERE id = 1;",
    hint: "Demonstrates write path and table refresh.",
  },
  {
    title: "Transaction path",
    sql: "BEGIN;\nINSERT INTO users VALUES (2, 'Grace', 31);\nUPDATE users SET age = 32 WHERE id = 2;\nSELECT * FROM users;\nCOMMIT;",
    hint: "Runs multiple statements to show buffered writes and commit.",
  },
];

const KEYWORDS = [
  "SELECT",
  "INSERT",
  "UPDATE",
  "DELETE",
  "CREATE TABLE",
  "CREATE INDEX",
  "DROP TABLE",
  "BEGIN",
  "COMMIT",
  "ROLLBACK",
  "FROM",
  "WHERE",
  "ORDER BY",
  "JOIN",
  "VALUES",
];

const PIPELINE = ["Lexer", "Parser", "Planner", "Txn", "B+Tree", "Result"];

function App() {
  const [sql, setSql] = useState(EXAMPLES[0].sql);
  const [state, setState] = useState({ ok: true, tables: [] });
  const [result, setResult] = useState(null);
  const [error, setError] = useState("");
  const [running, setRunning] = useState(false);
  const [activeStage, setActiveStage] = useState(-1);
  const [history, setHistory] = useState([]);
  const [selectedTable, setSelectedTable] = useState("");
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
  const textareaRef = useRef(null);

  const tableNames = useMemo(
    () => state.tables?.map((table) => table.meta.tableName) ?? [],
    [state],
  );

  const selected = useMemo(() => {
    if (!state.tables?.length) return null;
    return (
      state.tables.find((table) => table.meta.tableName === selectedTable) ??
      state.tables[0]
    );
  }, [state, selectedTable]);

  const suggestions = useMemo(() => {
    const words = [...KEYWORDS, ...tableNames];
    const tail = sql.split(/\s+/).pop()?.replace(/[;,()]/g, "") ?? "";
    if (!tail) return words.slice(0, 8);
    return words
      .filter((word) => word.toLowerCase().startsWith(tail.toLowerCase()))
      .slice(0, 8);
  }, [sql, tableNames]);

  useEffect(() => {
    refreshState();
  }, []);

  useEffect(() => {
    if (!selectedTable && state.tables?.length) {
      setSelectedTable(state.tables[0].meta.tableName);
    }
  }, [state, selectedTable]);

  async function refreshState() {
    const response = await fetch(`${API_BASE}/api/state`);
    const data = await response.json();
    setState(data);
  }

  async function executeSql() {
    const statements = sql
      .split(";")
      .map((part) => part.trim())
      .filter(Boolean);
    if (!statements.length) return;

    setRunning(true);
    setError("");
    setResult(null);
    setActiveStage(0);

    let lastPayload = null;
    for (const statement of statements) {
      await animatePipeline();
      const response = await fetch(`${API_BASE}/api/query`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sql: statement }),
      });
      const payload = await response.json();
      lastPayload = payload;
      setState(payload.state ?? state);
      setHistory((items) =>
        [
          {
            sql: statement,
            ok: payload.ok,
            at: new Date().toLocaleTimeString(),
          },
          ...items,
        ].slice(0, 8),
      );
      if (!payload.ok) {
        setError(payload.error || "Query failed");
        break;
      }
    }

    setResult(lastPayload?.result ?? null);
    setRunning(false);
    setActiveStage(-1);
  }

  async function animatePipeline() {
    for (let index = 0; index < PIPELINE.length; index += 1) {
      setActiveStage(index);
      await new Promise((resolve) => setTimeout(resolve, 130));
    }
  }

  function insertSuggestion(value) {
    const parts = sql.split(/(\s+)/);
    const lastIndex = parts.length - 1;
    parts[lastIndex] = value;
    setSql(parts.join(""));
    setSuggestionsOpen(false);
    textareaRef.current?.focus();
  }

  function resetDemo() {
    setSql(EXAMPLES[0].sql);
    setResult(null);
    setError("");
    setHistory([]);
  }

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <Database size={26} />
          <div>
            <h1>KV Demo</h1>
            <p>SQL to storage visual console</p>
          </div>
        </div>

        <section className="panel guide">
          <div className="panel-title">
            <HelpCircle size={17} />
            <span>Guided Flow</span>
          </div>
          {EXAMPLES.map((item, index) => (
            <button
              key={item.title}
              className="example"
              onClick={() => setSql(item.sql)}
            >
              <span className="step">{index + 1}</span>
              <span>
                <strong>{item.title}</strong>
                <small>{item.hint}</small>
              </span>
            </button>
          ))}
        </section>

        <section className="panel history">
          <div className="panel-title">
            <Activity size={17} />
            <span>Run History</span>
          </div>
          {history.length === 0 ? (
            <p className="muted">No statements executed yet.</p>
          ) : (
            history.map((item) => (
              <div className="history-row" key={`${item.at}-${item.sql}`}>
                <span className={item.ok ? "dot ok" : "dot bad"} />
                <span>{item.sql}</span>
                <time>{item.at}</time>
              </div>
            ))
          )}
        </section>
      </aside>

      <section className="workspace">
        <header className="toolbar">
          <div>
            <p className="eyebrow">Interactive SQL</p>
            <h2>Execute queries and watch the database change</h2>
          </div>
          <div className="actions">
            <button className="ghost" onClick={resetDemo}>
              <RotateCcw size={16} />
              Reset UI
            </button>
            <button className="primary" onClick={executeSql} disabled={running}>
              <Play size={16} />
              {running ? "Running" : "Run"}
            </button>
          </div>
        </header>

        <section className="editor-card">
          <div className="editor-head">
            <TerminalSquare size={18} />
            <span>SQL Editor</span>
            <button
              className="hint-button"
              onClick={() => setSuggestionsOpen((open) => !open)}
            >
              <Sparkles size={15} />
              Suggestions
            </button>
          </div>
          <textarea
            ref={textareaRef}
            value={sql}
            onChange={(event) => {
              setSql(event.target.value);
              setSuggestionsOpen(true);
            }}
            onFocus={() => setSuggestionsOpen(true)}
            spellCheck="false"
          />
          {suggestionsOpen && suggestions.length > 0 && (
            <div className="suggestions">
              {suggestions.map((item) => (
                <button key={item} onMouseDown={() => insertSuggestion(item)}>
                  {item}
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="pipeline">
          {PIPELINE.map((stage, index) => (
            <div
              key={stage}
              className={`stage ${activeStage === index ? "active" : ""} ${
                activeStage > index ? "done" : ""
              }`}
            >
              <span>{stage}</span>
              {index < PIPELINE.length - 1 && <ArrowRight size={16} />}
            </div>
          ))}
        </section>

        <section className="grid">
          <ResultView result={result} error={error} />
          <StateView
            state={state}
            selected={selected}
            selectedTable={selectedTable}
            setSelectedTable={setSelectedTable}
          />
        </section>
      </section>
    </main>
  );
}

function ResultView({ result, error }) {
  return (
    <section className="panel result-panel">
      <div className="panel-title">
        <TerminalSquare size={17} />
        <span>Result</span>
      </div>
      {error ? <div className="error">{error}</div> : null}
      {!error && !result ? <p className="muted">Run a statement to see output.</p> : null}
      {!error && result?.columns?.length === 0 ? (
        <div className="ok-card">Affected rows: {result.affectedRows}</div>
      ) : null}
      {!error && result?.columns?.length > 0 ? (
        <DataTable columns={result.columns} rows={result.rows} />
      ) : null}
    </section>
  );
}

function StateView({ state, selected, selectedTable, setSelectedTable }) {
  return (
    <section className="panel state-panel">
      <div className="panel-title">
        <Table2 size={17} />
        <span>Database State</span>
      </div>
      {!state.tables?.length ? (
        <p className="muted">Create a table to populate the visual state.</p>
      ) : (
        <>
          <div className="tabs">
            {state.tables.map((table) => (
              <button
                key={table.meta.tableName}
                className={selectedTable === table.meta.tableName ? "selected" : ""}
                onClick={() => setSelectedTable(table.meta.tableName)}
              >
                {table.meta.tableName}
              </button>
            ))}
          </div>
          <div className="schema-strip">
            {selected.meta.columns.map((column) => (
              <span key={column.name}>
                {column.name}
                <small>{column.dataType}</small>
              </span>
            ))}
          </div>
          <DataTable columns={selected.meta.columns} rows={selected.rows} compact />
        </>
      )}
    </section>
  );
}

function DataTable({ columns, rows, compact = false }) {
  return (
    <div className={`table-wrap ${compact ? "compact" : ""}`}>
      <table>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.name}>{column.name}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={columns.length || 1}>No rows</td>
            </tr>
          ) : (
            rows.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {columns.map((column, colIndex) => (
                  <td key={column.name}>{String(row[colIndex] ?? "NULL")}</td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

export default App;
