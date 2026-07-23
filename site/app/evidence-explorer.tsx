"use client";

import { useState } from "react";

type QuestionKey = "launch" | "monday" | "old-info";
type ViewKey = "then" | "now";

const questions: Record<
  QuestionKey,
  {
    prompt: string;
    then: string;
    now: string;
    status: string;
    ratified: string;
    receipts: Array<{ type: string; title: string; detail: string }>;
    counterpoint: string;
    coverage: string;
    gap: string;
    timestamp: string;
  }
> = {
  launch: {
    prompt: "Why did the Atlas launch move?",
    then:
      "The launch moved to August 7 because the rollback rehearsal failed twice and the team chose recoverability over a feature-flag-only release.",
    now:
      "August 7 still stands. Rollback automation passed this morning, but the release remains gated on the final recovery runbook review.",
    status: "Ratified decision",
    ratified: "Ratified by Maya + Theo / 10:42",
    receipts: [
      { type: "Huddle", title: "Release readiness", detail: "24:18–27:04" },
      { type: "Thread", title: "#atlas-launch", detail: "Events 312–329" },
      { type: "Workflow", title: "rollback-rehearsal", detail: "Failed ×2" },
      { type: "Code", title: "PR #482", detail: "Deployment guard" },
    ],
    counterpoint: "Aaron preferred shipping July 31 behind a feature flag.",
    coverage: "92% of accessible project activity examined",
    gap: "Private leadership room was not inspected.",
    timestamp: "True as of Jul 22 / 10:42",
  },
  monday: {
    prompt: "What changed since Monday?",
    then:
      "The release was green on product readiness but blocked on rollback proof and an unowned customer migration checklist.",
    now:
      "Rollback proof is complete. Priya accepted ownership of the migration checklist, leaving one open recovery runbook review.",
    status: "Storyline update",
    ratified: "Confirmed across 3 project surfaces",
    receipts: [
      { type: "Workflow", title: "recovery-suite", detail: "Passed / 09:14" },
      { type: "Thread", title: "#atlas-release", detail: "Owner accepted" },
      { type: "Artifact", title: "Migration checklist", detail: "Revision 6" },
      { type: "Decision", title: "Release gate", detail: "1 item remains" },
    ],
    counterpoint: "Product still considers the release ready from a customer-facing standpoint.",
    coverage: "88% of accessible launch activity examined",
    gap: "Two linked external tickets were unavailable.",
    timestamp: "Compared Jul 20 → Jul 22",
  },
  "old-info": {
    prompt: "Who is working from old information?",
    then:
      "Design and Support are still planning around the July 31 launch date that Engineering superseded yesterday.",
    now:
      "Support acknowledged the new date. Design has not yet viewed the ratified decision and one dependent milestone is still stale.",
    status: "Communication gap",
    ratified: "Derived from access-aware activity",
    receipts: [
      { type: "Plan", title: "Design milestones", detail: "References Jul 31" },
      { type: "Decision", title: "Launch moved", detail: "Ratified Aug 7" },
      { type: "Presence", title: "#atlas-launch", detail: "Design unread" },
      { type: "Thread", title: "#support", detail: "Acknowledged" },
    ],
    counterpoint: "Design may have received the update in a source Comb cannot access.",
    coverage: "81% of accessible cross-team activity examined",
    gap: "Direct messages and private design rooms were not inspected.",
    timestamp: "True as of Jul 22 / 13:08",
  },
};

const questionOrder = Object.keys(questions) as QuestionKey[];

export function EvidenceExplorer() {
  const [selected, setSelected] = useState<QuestionKey>("launch");
  const [view, setView] = useState<ViewKey>("now");
  const [showReceipts, setShowReceipts] = useState(true);
  const [showGap, setShowGap] = useState(false);
  const answer = questions[selected];

  return (
    <div className="explorer">
      <div className="question-panel">
        <span className="demo-label">Interactive demo / illustrative data</span>
        <p>Ask the project</p>
        <div className="question-list" role="list" aria-label="Example questions">
          {questionOrder.map((key) => (
            <button
              aria-pressed={selected === key}
              className={selected === key ? "selected" : ""}
              key={key}
              onClick={() => {
                setSelected(key);
                setShowGap(false);
              }}
              type="button"
            >
              <span aria-hidden="true">{selected === key ? "●" : "○"}</span>
              {questions[key].prompt}
            </button>
          ))}
        </div>
        <div className="view-toggle" aria-label="Answer timeframe">
          <button aria-pressed={view === "then"} onClick={() => setView("then")} type="button">
            As decided
          </button>
          <button aria-pressed={view === "now"} onClick={() => setView("now")} type="button">
            Now
          </button>
        </div>
      </div>

      <article className="memory-receipt" aria-live="polite" aria-atomic="true">
        <header>
          <div>
            <span>Comb / Memory receipt</span>
            <strong>{answer.status}</strong>
          </div>
          <span>{answer.timestamp}</span>
        </header>

        <div className="receipt-answer">
          <span>{view === "then" ? "As decided" : "What is true now"}</span>
          <h3>{view === "then" ? answer.then : answer.now}</h3>
          <p>{answer.ratified}</p>
        </div>

        <div className="receipt-actions">
          <button
            aria-expanded={showReceipts}
            onClick={() => setShowReceipts((value) => !value)}
            type="button"
          >
            {showReceipts ? "Hide the receipts" : "Show the receipts"}
          </button>
          <button aria-expanded={showGap} onClick={() => setShowGap((value) => !value)} type="button">
            {showGap ? "Hide the gap" : "Show the gap"}
          </button>
        </div>

        {showReceipts ? (
          <div className="receipt-grid">
            {answer.receipts.map((receipt, index) => (
              <div key={`${selected}-${receipt.type}`}>
                <span>{String(index + 1).padStart(2, "0")} / {receipt.type}</span>
                <strong>{receipt.title}</strong>
                <p>{receipt.detail}</p>
              </div>
            ))}
          </div>
        ) : null}

        <div className="receipt-context">
          <div>
            <span>Counterpoint</span>
            <p>{answer.counterpoint}</p>
          </div>
          <div>
            <span>Coverage</span>
            <p>{answer.coverage}</p>
          </div>
          {showGap ? (
            <div className="known-gap">
              <span>Known gap</span>
              <p>{answer.gap}</p>
            </div>
          ) : null}
        </div>

        <footer>
          <span>{answer.receipts.length} receipts / 1 known gap / 1 dissenting view</span>
          <strong>Evidence attached ✓</strong>
        </footer>
      </article>
    </div>
  );
}
