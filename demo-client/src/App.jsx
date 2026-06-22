import {
  Activity,
  ArrowRight,
  Blocks,
  Database,
  HelpCircle,
  Layers3,
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
    hint: "Start with catalog metadata and a primary-key layout.",
  },
  {
    title: "Insert seed row",
    sql: "INSERT INTO users VALUES (1, 'Ada', 28);",
    hint: "Watch the row appear in the storage view.",
  },
  {
    title: "Projection query",
    sql: "SELECT name, age FROM users WHERE id = 1;",
    hint: "Compare selected columns against table state.",
  },
  {
    title: "Update row",
    sql: "UPDATE users SET age = 29 WHERE id = 1;",
    hint: "Show a write path and refreshed data block.",
  },
  {
    title: "Transaction path",
    sql: "BEGIN;\nINSERT INTO users VALUES (2, 'Grace', 31);\nUPDATE users SET age = 32 WHERE id = 2;\nSELECT * FROM users;\nCOMMIT;",
    hint: "Demonstrate buffered writes before commit.",
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

const PIPELINE = [
  { name: "Lexer", detail: "SQL text -> tokens" },
  { name: "Parser", detail: "Tokens -> AST" },
  { name: "Planner", detail: "AST -> plan nodes" },
  { name: "Txn", detail: "Locks and buffered writes" },
  { name: "B+Tree", detail: "Primary-key storage" },
  { name: "Result", detail: "Rows or affected count" },
];

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
  const [lastStatement, setLastStatement] = useState("");
  const textareaRef = useRef(null);

  const tables = state.tables ?? [];
  const tableNames = useMemo(
    () => tables.map((table) => table.meta.tableName),
    [tables],
  );
  const totalRows = tables.reduce((sum, table) => sum + table.rows.length, 0);
  const totalColumns = tables.reduce(
    (sum, table) => sum + table.meta.columns.length,
    0,
  );

  const selected = useMemo(() => {
    if (!tables.length) return null;
    return (
      tables.find((table) => table.meta.tableName === selectedTable) ??
      tables[0]
    );
  }, [tables, selectedTable]);

  const suggestions = useMemo(() => {
    const words = [...KEYWORDS, ...tableNames];
    const tail = sql.split(/\s+/).pop()?.replace(/[;,()]/g, "") ?? "";
    if (!tail) return words.slice(0, 8);
    return words
      .filter((word) => word.toLowerCase().startsWith(tail.toLowerCase()))
      .slice(0, 8);
  }, [sql, tableNames]);

  const statementCount = sql
    .split(";")
    .map((part) => part.trim())
    .filter(Boolean).length;

  useEffect(() => {
    refreshState();
  }, []);

  useEffect(() => {
    if (!selectedTable && tables.length) {
      setSelectedTable(tables[0].meta.tableName);
    }
  }, [tables, selectedTable]);

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
      setLastStatement(statement);
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
      await new Promise((resolve) => setTimeout(resolve, 150));
    }
  }

  function insertSuggestion(value) {
    const parts = sql.split(/(\s+)/);
    parts[parts.length - 1] = value;
    setSql(parts.join(""));
    setSuggestionsOpen(false);
    textareaRef.current?.focus();
  }

  function resetDemo() {
    setSql(EXAMPLES[0].sql);
    setResult(null);
    setError("");
    setHistory([]);
    setLastStatement("");
  }

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <Database size={26} />
          <div>
            <h1>KV Demo</h1>
            <p>Relational SQL on a Rust KV engine</p>
          </div>
        </div>

        <section className="side-card metrics">
          <Metric label="Tables" value={tables.length} />
          <Metric label="Rows" value={totalRows} />
          <Metric label="Columns" value={totalColumns} />
        </section>

        <section className="side-card guide">
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

        <section className="side-card history">
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
        <header className="hero">
          <div>
            <p className="eyebrow">Interactive SQL Console</p>
            <h2>Run a statement, then trace how data moves through the engine.</h2>
          </div>
          <div className="actions">
            <button className="ghost" onClick={resetDemo}>
              <RotateCcw size={16} />
              Reset UI
            </button>
            <button className="primary" onClick={executeSql} disabled={running}>
              <Play size={16} />
              {running ? "Running" : `Run ${statementCount || ""}`}
            </button>
          </div>
        </header>

        <section className="top-grid">
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

          <Pipeline activeStage={activeStage} lastStatement={lastStatement} />
        </section>

        <section className="content-grid">
          <ResultView result={result} error={error} />
          <StateView
            tables={tables}
            selected={selected}
            selectedTable={selectedTable}
            setSelectedTable={setSelectedTable}
          />
          <StorageView selected={selected} />
        </section>
      </section>
    </main>
  );
}

function Metric({ label, value }) {
  return (
    <div className="metric">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

function Pipeline({ activeStage, lastStatement }) {
  return (
    <section className="pipeline-card">
      <div className="panel-title">
        <Layers3 size={17} />
        <span>Execution Flow</span>
      </div>
      <div className="statement-chip">
        {lastStatement || "Waiting for the next statement"}
      </div>
      <div className="pipeline">
        {PIPELINE.map((stage, index) => (
          <div
            key={stage.name}
            className={`stage ${activeStage === index ? "active" : ""} ${
              activeStage > index ? "done" : ""
            }`}
          >
            <strong>{stage.name}</strong>
            <small>{stage.detail}</small>
            {index < PIPELINE.length - 1 && <ArrowRight size={15} />}
          </div>
        ))}
      </div>
    </section>
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
      {!error && !result ? (
        <p className="muted">Run a statement to see output.</p>
      ) : null}
      {!error && result?.columns?.length === 0 ? (
        <div className="ok-card">
          <strong>{result.affectedRows}</strong>
          <span>affected rows</span>
        </div>
      ) : null}
      {!error && result?.columns?.length > 0 ? (
        <DataTable columns={result.columns} rows={result.rows} />
      ) : null}
    </section>
  );
}

function StateView({ tables, selected, selectedTable, setSelectedTable }) {
  return (
    <section className="panel state-panel">
      <div className="panel-title">
        <Table2 size={17} />
        <span>Table State</span>
      </div>
      {!tables.length ? (
        <EmptyState />
      ) : (
        <>
          <div className="tabs">
            {tables.map((table) => (
              <button
                key={table.meta.tableName}
                className={selectedTable === table.meta.tableName ? "selected" : ""}
                onClick={() => setSelectedTable(table.meta.tableName)}
              >
                {table.meta.tableName}
                <small>{table.rows.length}</small>
              </button>
            ))}
          </div>
          <div className="schema-strip">
            {selected.meta.columns.map((column, index) => (
              <span key={column.name} className={column.primaryKey ? "pk" : ""}>
                {index === selected.meta.primaryKeyIndex ? "PK " : ""}
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

function StorageView({ selected }) {
  const columns = selected?.meta.columns ?? [];
  const rows = selected?.rows ?? [];

  return (
    <section className="panel storage-panel">
      <div className="panel-title">
        <Blocks size={17} />
        <span>Storage Blocks</span>
      </div>
      {!selected ? (
        <p className="muted">Storage blocks appear after a table exists.</p>
      ) : rows.length === 0 ? (
        <p className="muted">The selected table has no rows yet.</p>
      ) : (
        <div className="block-grid">
          {rows.map((row, rowIndex) => (
            <div className="data-block" key={rowIndex}>
              <div className="block-head">
                key
                <strong>{String(row[selected.meta.primaryKeyIndex] ?? rowIndex)}</strong>
              </div>
              {columns.map((column, columnIndex) => (
                <div className="block-field" key={column.name}>
                  <span>{column.name}</span>
                  <strong>{String(row[columnIndex] ?? "NULL")}</strong>
                </div>
              ))}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function EmptyState() {
  return (
    <div className="empty-state">
      <Database size={28} />
      <strong>No tables yet</strong>
      <span>Use the first guide step to create a table.</span>
    </div>
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
