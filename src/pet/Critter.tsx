export function Critter({ badge }: { badge: string }) {
  return (
    <svg className="critter" viewBox="0 0 120 120" width="140" height="140" aria-hidden="true">
      <line className="branch" x1="8" y1="108" x2="112" y2="108" />
      <g className="feet">
        <path d="M50 100 v8 M46 108 h8" />
        <path d="M68 100 v8 M64 108 h8" />
      </g>
      <g className="critter-body">
        <ellipse className="tail" cx="92" cy="80" rx="14" ry="7" transform="rotate(-25 92 80)" />
        <ellipse className="body" cx="60" cy="70" rx="34" ry="32" />
        <ellipse className="belly" cx="56" cy="80" rx="20" ry="18" />
        <path className="tuft" d="M56 40 q4 -12 10 -4 q2 -8 8 -2" />
        <ellipse className="wing" cx="82" cy="72" rx="10" ry="16" />
        <g className="eye">
          <circle className="eye-white" cx="48" cy="60" r="7" />
          <circle className="pupil" cx="49" cy="61" r="3.5" />
        </g>
        <g className="eye">
          <circle className="eye-white" cx="68" cy="60" r="7" />
          <circle className="pupil" cx="69" cy="61" r="3.5" />
        </g>
        <path className="beak" d="M54 68 l6 8 l6 -8 z" />
      </g>
      <g className="zzz">
        <text x="88" y="30">z</text>
        <text x="98" y="18">z</text>
      </g>
      {badge && (
        <g className="badge">
          <circle cx="100" cy="30" r="12" />
          <text x="100" y="35" textAnchor="middle">{badge}</text>
        </g>
      )}
    </svg>
  );
}
