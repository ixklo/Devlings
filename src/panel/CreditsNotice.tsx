export function CreditsNotice({ petName, onOk, onCancel }: { petName: string; onOk: () => void; onCancel: () => void }) {
  return (
    <div className="modal" role="dialog" aria-modal="true" aria-labelledby="credits-title">
      <div className="modal-card">
        <h2 id="credits-title">Before your first ask</h2>
        <p>
          {petName} runs Claude Code on your subscription, never an API key. Asks use your plan's usage like any other
          prompt.
        </p>
        <p>
          If you turned on usage credits in your Claude account, {petName} stops any run the moment it would start
          using them.
        </p>
        <div className="row">
          <button className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button onClick={onOk}>Got it</button>
        </div>
      </div>
    </div>
  );
}
