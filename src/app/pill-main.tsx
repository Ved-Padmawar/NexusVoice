import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import "../index.css";
import { PillApp } from "../pill/PillApp";
import { ErrorBoundary } from "./ErrorBoundary";
import { queryClient } from "./queryClient";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <PillApp />
      </QueryClientProvider>
    </ErrorBoundary>
  </StrictMode>,
);
