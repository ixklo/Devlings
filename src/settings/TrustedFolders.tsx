import { api } from "../shared/api";
import type { ProjectEntry } from "../shared/types";
import { Section } from "./controls";
import type { useAction } from "./useAction";

interface Props {
  projects: ProjectEntry[];
  petName: string;
  action: ReturnType<typeof useAction>;
}

/**
 * Folders trusted in Perch (v1.0 D6), each with Stop trusting. Hidden while there are none; folders get here from
 * the "Trust this folder" button on an untrusted-folder notice in the mini chat.
 */
export function TrustedFolders({ projects, petName, action }: Props) {
  const trusted = projects.filter((p) => p.trusted);
  if (trusted.length === 0) return null;
  return (
    <Section
      title="Trusted folders"
      description={`Asks in these folders use the folder's own Claude Code settings: its hooks, environment variables, MCP servers and skills, even though Claude Code hasn't trusted it. Stop trusting one and ${petName} skips them again.`}
    >
      {trusted.map((p) => (
        <div className="row" key={p.path}>
          <div className="row-text">
            <span className="row-label">{p.name}</span>
            <p className="row-desc mono-path" title={p.path}>
              {p.path}
            </p>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            aria-label={`Stop trusting ${p.name}`}
            disabled={action.busy}
            onClick={() => void action.run(() => api.untrustProject(p.path), `Stopped trusting ${p.name}.`)}
          >
            Stop trusting
          </button>
        </div>
      ))}
    </Section>
  );
}
