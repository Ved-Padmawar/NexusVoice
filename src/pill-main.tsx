import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { PillApp } from "./pill/PillApp";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PillApp />
  </StrictMode>,
);
