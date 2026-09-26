import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// vitest.config's `test.globals` is off, so Testing Library's automatic
// afterEach cleanup (which detects a global `afterEach`) never registers.
// Register it explicitly so DOM from one test doesn't leak into the next.
afterEach(cleanup);
