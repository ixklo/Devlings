import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../shared/api";
import type { ProjectEntry } from "../shared/types";

const CHOOSE = "__choose__";

interface Props {
  projects: ProjectEntry[];
  value: string | null;
  onChange: (path: string) => void;
  onError: (message: string) => void;
}

export function ProjectPicker({ projects, value, onChange, onError }: Props) {
  const choose = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    try {
      await api.addProject(dir);
      onChange(dir);
    } catch (e) {
      onError(String(e));
    }
  };
  return (
    <select
      aria-label="Project"
      value={value ?? ""}
      onChange={(e) => (e.target.value === CHOOSE ? choose() : onChange(e.target.value))}
    >
      {!value && <option value="">Pick a project…</option>}
      {projects.map((p) => (
        <option key={p.path} value={p.path} title={p.path}>
          {p.name}
        </option>
      ))}
      <option value={CHOOSE}>Choose folder…</option>
    </select>
  );
}
