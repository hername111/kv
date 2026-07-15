import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App.jsx";
import "./styles.css";

class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <main className="fatal">
          <h1>KV Database 启动失败</h1>
          <p>{this.state.error.message}</p>
        </main>
      );
    }
    return this.props.children;
  }
}

window.addEventListener("error", (event) => {
  const root = document.getElementById("root");
  if (root && root.childElementCount === 0) {
    const main = document.createElement("main");
    const title = document.createElement("h1");
    const detail = document.createElement("p");
    main.className = "fatal";
    title.textContent = "KV Database 启动失败";
    detail.textContent = event.message;
    main.append(title, detail);
    root.replaceChildren(main);
  }
});

createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
