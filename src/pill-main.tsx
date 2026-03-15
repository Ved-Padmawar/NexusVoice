import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { PillApp } from "./pill/PillApp";
import { ErrorBoundary } from "./components/ErrorBoundary";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <PillApp />
    </ErrorBoundary>
  </StrictMode>,
);
