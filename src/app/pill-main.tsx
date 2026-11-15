import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import "../index.css";
import { PillApp } from "../pill/PillApp";
import { logger } from "../shared/logger";
import { ErrorBoundary } from "./ErrorBoundary";
import { queryClient } from "./queryClient";
import { mirrorRust } from "./mirror";

const render = () => createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <PillApp />
      </QueryClientProvider>
    </ErrorBoundary>
  </StrictMode>,
);

mirrorRust()
  .catch((e: unknown) => logger.error("could not mirror the backend state", e))
  .finally(render);
