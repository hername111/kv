import {
  Activity,
  ArrowRight,
  Blocks,
  CircleCheck,
  CircleX,
  Database,
  Layers3,
  Play,
  RefreshCw,
  RotateCcw,
  Table2,
  TerminalSquare,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

const API_BASE = "";

const EXAMPLES = [
  {
    title: "创建用户表",
    sql: "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT);",
  },
  {
    title: "插入示例数据",
    sql: "INSERT INTO users VALUES (1, 'Ada', 28);",
  },
  {
    title: "条件查询",
    sql: "SELECT name, age FROM users WHERE id = 1;",
  },
  {
    title: "更新记录",
    sql: "UPDATE users SET age = 29 WHERE id = 1;",
  },
  {
    title: "事务操作",
    sql: "BEGIN;\nINSERT INTO users VALUES (2, 'Grace', 31);\nUPDATE users SET age = 32 WHERE id = 2;\nSELECT * FROM users;\nCOMMIT;",
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
  { name: "词法", detail: "SQL -> Token" },
  { name: "语法", detail: "Token -> AST" },
  { name: "计划", detail: "AST -> Plan" },
  { name: "事务", detail: "锁与写集" },
  { name: "B+Tree", detail: "页面索引" },
  { name: "结果", detail: "结果集" },
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
  const [connected, setConnected] = useState(null);
  const [duration, setDuration] = useState(null);
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
    try {
      const response = await fetch(`${API_BASE}/api/state`);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data = await response.json();
      setState(data);
      setConnected(Boolean(data.ok));
      setError("");
    } catch (requestError) {
      setConnected(false);
      setError(`无法连接数据库服务：${requestError.message}`);
    }
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
    setDuration(null);
    setActiveStage(0);
    const startedAt = performance.now();

    let lastPayload = null;
    try {
      for (const statement of statements) {
        setLastStatement(statement);
        await animatePipeline();
        const response = await fetch(`${API_BASE}/api/query`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ sql: statement }),
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const payload = await response.json();
        lastPayload = payload;
        if (payload.state) setState(payload.state);
        setConnected(true);
        setHistory((items) =>
          [
            {
              sql: statement,
              ok: payload.ok,
              at: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
            },
            ...items,
          ].slice(0, 8),
        );
        if (!payload.ok) {
          setError(payload.error || "SQL 执行失败");
          break;
        }
      }
      setResult(lastPayload?.result ?? null);
    } catch (requestError) {
      setConnected(false);
      setError(`请求失败：${requestError.message}`);
    } finally {
      setDuration(Math.round(performance.now() - startedAt));
      setRunning(false);
      setActiveStage(-1);
    }
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
    setDuration(null);
  }

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <Database size={26} />
          <div>
            <h1>KV Database</h1>
            <p>Rust 关系型数据库工作台</p>
          </div>
        </div>

        <section className="side-card metrics">
          <Metric label="数据表" value={tables.length} />
          <Metric label="记录" value={totalRows} />
          <Metric label="字段" value={totalColumns} />
        </section>

        <section className="side-card guide">
          <div className="panel-title">
            <TerminalSquare size={17} />
            <span>SQL 模板</span>
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
              </span>
            </button>
          ))}
        </section>

        <section className="side-card history">
          <div className="panel-title">
            <Activity size={17} />
            <span>执行历史</span>
          </div>
          {history.length === 0 ? (
            <p className="muted">暂无执行记录</p>
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
            <p className="eyebrow">KV DATABASE STUDIO</p>
            <div className={`connection ${connected ? "online" : connected === false ? "offline" : "checking"}`}>
              {connected ? <CircleCheck size={15} /> : <CircleX size={15} />}
              {connected ? "服务在线" : connected === false ? "服务离线" : "正在连接"}
            </div>
          </div>
          <div className="actions">
            <button className="icon-button" onClick={refreshState} title="刷新数据库状态" aria-label="刷新数据库状态">
              <RefreshCw size={16} />
            </button>
            <button className="ghost" onClick={resetDemo}>
              <RotateCcw size={16} />
              重置
            </button>
            <button className="primary" onClick={executeSql} disabled={running}>
              <Play size={16} />
              {running ? "执行中" : `执行${statementCount > 1 ? ` ${statementCount} 条` : ""}`}
            </button>
          </div>
        </header>

        <section className="top-grid">
          <section className="editor-card">
            <div className="editor-head">
              <TerminalSquare size={18} />
              <span>SQL 编辑器</span>
              <button
                className="icon-button subtle"
                onClick={() => setSuggestionsOpen((open) => !open)}
                title="SQL 补全"
                aria-label="SQL 补全"
              >
                <TerminalSquare size={15} />
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
              onKeyDown={(event) => {
                if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
                  event.preventDefault();
                  executeSql();
                }
              }}
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
          <ResultView result={result} error={error} duration={duration} />
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
        <span>执行链路</span>
      </div>
      <div className="statement-chip">
        {lastStatement || "等待执行"}
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

function ResultView({ result, error, duration }) {
  return (
    <section className="panel result-panel">
      <div className="panel-title">
        <TerminalSquare size={17} />
        <span>执行结果</span>
        {duration !== null ? <small className="duration">{duration} ms</small> : null}
      </div>
      {error ? <div className="error">{error}</div> : null}
      {!error && !result ? (
        <p className="muted">暂无结果</p>
      ) : null}
      {!error && result?.columns?.length === 0 ? (
        <div className="ok-card">
          <strong>{result.affectedRows}</strong>
          <span>行受影响</span>
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
        <span>数据表状态</span>
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
        <span>存储记录</span>
      </div>
      {!selected ? (
        <p className="muted">暂无数据表</p>
      ) : rows.length === 0 ? (
        <p className="muted">当前表暂无记录</p>
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
      <strong>暂无数据表</strong>
      <span>数据库目录当前为空</span>
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
              <td colSpan={columns.length || 1}>暂无记录</td>
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
